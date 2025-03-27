pub mod active_device;
pub mod active_parameter;
pub mod sink;
pub mod stream;
#[cfg(not(target_arch = "wasm32"))]
pub mod tcpstream;
pub mod websocket;

use crate::{
    device,
    serialization::device::{Device, DeviceUpdate},
};
use derive_more::{Deref, DerefMut, From};
use std::{fmt::Debug, future::Future};
use tokio::sync::{mpsc::UnboundedReceiver, oneshot, Mutex};
use tokio_stream::StreamExt;
use tracing::{error, info, Instrument};
use twinkle_client::{
    self,
    task::{spawn_with_state, AsyncTask},
    MaybeSend,
};

use std::{
    collections::HashMap,
    sync::{Arc, PoisonError},
    time::Duration,
};

use crate::{
    serialization, Command, DeError, GetProperties, Parameter, TypeError, UpdateError,
    INDI_PROTOCOL_VERSION,
};
pub use twinkle_client::notify::{self, wait_fn, Notify};

#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
    SendError(active_device::SendError<Command>),
    Canceled,
    Timeout,
    EndOfStream,
    PropertyError,
    TypeMismatch,
    PoisonError,
}

impl<T> From<notify::Error<ChangeError<T>>> for ChangeError<T> {
    fn from(value: notify::Error<ChangeError<T>>) -> Self {
        match value {
            notify::Error::Timeout => ChangeError::Timeout,
            notify::Error::Canceled => ChangeError::Canceled,
            notify::Error::EndOfStream => ChangeError::EndOfStream,
            notify::Error::Abort(e) => e,
        }
    }
}

impl<E> From<std::io::Error> for ChangeError<E> {
    fn from(value: std::io::Error) -> Self {
        ChangeError::<E>::IoError(value)
    }
}
impl<E> From<active_device::SendError<Command>> for ChangeError<E> {
    fn from(value: active_device::SendError<Command>) -> Self {
        ChangeError::<E>::SendError(value)
    }
}
impl<E> From<notify::Error<E>> for ChangeError<E> {
    fn from(value: notify::Error<E>) -> Self {
        ChangeError::NotifyError(value)
    }
}
impl<E> From<DeError> for ChangeError<E> {
    fn from(value: DeError) -> Self {
        ChangeError::<E>::DeError(value)
    }
}
impl<E> From<TypeError> for ChangeError<E> {
    fn from(_: TypeError) -> Self {
        ChangeError::<E>::TypeMismatch
    }
}
impl<E> From<crossbeam_channel::SendError<Command>> for ChangeError<E> {
    fn from(value: crossbeam_channel::SendError<Command>) -> Self {
        ChangeError::Disconnected(value)
    }
}

impl<E, T> From<PoisonError<T>> for ChangeError<E> {
    fn from(_: PoisonError<T>) -> Self {
        ChangeError::PoisonError
    }
}

#[derive(From, Deref, DerefMut)]
pub struct ClientTask<S>(AsyncTask<(), S>);

/// Create a new Client object that will stay in sync with the INDI server
/// on the other end of `connection`.
///
/// # Arguments
/// * `connection` - An object implementing `ClientConnection` (such as TcpStream) that will be used
///   to communicate with an INDI server.
/// * `device` - An optional name for a specific device to track.  If a value is provided only parameters
///   from that device will be available from `get_device()`.
/// * `parameter` - An optional name for the given `device`'s parameter to track.
///

pub fn new<T: AsyncClientConnection>(
    connection: T,
    device: Option<&str>,
    parameter: Option<&str>,
) -> ClientTask<Arc<Mutex<Client>>> {
    let (feedback, commands) = tokio::sync::mpsc::unbounded_channel::<Command>();
    let client = Client {
        devices: Default::default(),
        connected: Arc::new(Notify::new(false)),
        feedback: Some(feedback),
    };
    let (writer, reader) = connection.to_indi();
    start_with_streams(client, commands, writer, reader, device, parameter)
}

pub fn new_with_streams(
    writer: impl AsyncWriteConnection + MaybeSend + 'static,
    reader: impl AsyncReadConnection + MaybeSend + 'static,
    device: Option<&str>,
    parameter: Option<&str>,
) -> ClientTask<Arc<Mutex<Client>>> {
    let (feedback, incoming_commands) = tokio::sync::mpsc::unbounded_channel::<Command>();
    let client = Client {
        devices: Default::default(),
        connected: Arc::new(Notify::new(false)),
        feedback: Some(feedback),
    };
    start_with_streams(client, incoming_commands, writer, reader, device, parameter)
}

pub fn start<T: AsyncClientConnection>(
    client: Client,
    incoming_commands: UnboundedReceiver<Command>,
    connection: T,
    device: Option<&str>,
    parameter: Option<&str>,
) -> ClientTask<Arc<Mutex<Client>>> {
    let (writer, reader) = connection.to_indi();
    start_with_streams(client, incoming_commands, writer, reader, device, parameter)
}

pub fn start_with_streams(
    client: Client,
    mut incoming_commands: UnboundedReceiver<Command>,
    mut writer: impl AsyncWriteConnection + MaybeSend + 'static,
    mut reader: impl AsyncReadConnection + MaybeSend + 'static,
    device: Option<&str>,
    parameter: Option<&str>,
) -> ClientTask<Arc<Mutex<Client>>> {
    let connected = client.connected.clone();
    let feedback = client.feedback.as_ref().unwrap().clone();

    let writer_device = device.map(|x| String::from(x));
    let writer_parameter = parameter.map(|x| String::from(x));
    let writer_connected = connected.clone();

    let (reader_finished_tx, reader_finished_rx) = oneshot::channel::<()>();
    feedback
        .send(serialization::Command::GetProperties(GetProperties {
            version: INDI_PROTOCOL_VERSION.to_string(),
            device: writer_device,
            name: writer_parameter,
        }))
        .unwrap();
    let writer_future = async move {
        let sender = async {
            loop {
                let command = match incoming_commands.recv().await {
                    Some(c) => c,
                    None => break,
                };
                writer.write(command).await?;
            }
            Ok::<(), DeError>(())
        }
        .await;
        if let Err(e) = sender {
            error!("Error in sending task: {:?}", e);
        }
        dbg!("writer loop done");

        if let Err(e) = writer.shutdown().await {
            error!("Error shutting down writer: {:?}", e);
        }
        dbg!("shutdown writer");
        let _ = reader_finished_rx.await;
        {
            *writer_connected.write().await = false;
        }
    }
    .instrument(tracing::info_span!("indi_writer"));
    let devices = client.devices.clone();
    let thread_devices = devices.clone();
    let reader_future = async move {
        loop {
            let command = match reader.read().await {
                Some(c) => c,
                None => break,
            };
            match command {
                Ok(command) => {
                    dbg!(&command);
                    let locked_devices = thread_devices.write().await;
                    dbg!("locked_devices");
                    let update_result = match command.param_update_type() {
                        serialization::ParamUpdateType::Add => {
                            dbg!("add");
                            let device_name = command.device_name().cloned();
                            if let Some(device_name) = device_name {
                                if locked_devices.contains_key(&device_name) {
                                    dbg!("add -> update");
                                    locked_devices.update(command).await
                                } else {
                                    dbg!("add -> create");
                                    let mut locked_devices = locked_devices;
                                    dbg!(locked_devices.create(command).await)
                                }
                            } else {
                                Ok(None)
                            }
                        }
                        serialization::ParamUpdateType::Update => {
                            locked_devices.update(command).await
                        }
                        serialization::ParamUpdateType::Remove => {
                            let device_name = command.device_name().cloned();

                            let update = locked_devices.update(command).await;
                            if let Some(device_name) = device_name {
                                if let Some(device) = locked_devices.get(&device_name) {
                                    if device.read().await.get_parameters().len() == 0 {
                                        let mut locked_devices = locked_devices;
                                        locked_devices.remove(&device_name);
                                    }
                                }
                            }

                            update
                        }
                        serialization::ParamUpdateType::Noop => Ok(None),
                    };
                    dbg!(&update_result);
                    if let Err(e) = update_result {
                        error!("Device update error: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Command error: {:?}", e);
                }
            }
        }
        *thread_devices.write().await = Default::default();
        let _ = reader_finished_tx.send(());
        info!("reader finished");
    }
    .instrument(tracing::info_span!("indi_reader"));

    let task = spawn_with_state(Arc::new(Mutex::new(client)), |_| async {
        tokio::select! {
            _ = writer_future => tracing::info!("writer_future finisehd"),
            _ = reader_future => tracing::info!("reader_future finisehd"),
        }
    });
    task.into()
}

/// Struct used to keep track of a the devices and their properties.
pub struct Client {
    devices: Arc<Notify<MemoryDeviceStore>>,
    connected: Arc<Notify<bool>>,
    feedback: Option<tokio::sync::mpsc::UnboundedSender<Command>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Client {
    pub fn new(feedback: Option<tokio::sync::mpsc::UnboundedSender<Command>>) -> Self {
        Client {
            devices: Default::default(),
            connected: Arc::new(Notify::new(true)),
            feedback,
        }
    }
    /// Async method that will wait up to 1 second for the device named `name` to be defined
    ///  by the INDI server.  The returned `ActiveDevice` (if present) will be associated with
    ///  the `self` client for communicating changes with the INDI server it came from.
    ///
    /// # Arguments
    /// * `name` - Name of device on the remote INDI server you wish to get.
    ///
    /// # Example
    /// ```no_run
    /// use tokio::net::TcpStream;
    /// use twinkle_client::task::Task;
    /// use std::ops::Deref;
    /// // Client that will track all devices and parameters to the connected INDI server at localhost.
    ///
    /// async {
    ///     let client_task = indi::client::new(TcpStream::connect("localhost:7624").await.expect("Connecting to server"), None, None);
    ///     let status = client_task.status();
    ///     if let twinkle_client::task::Status::Running(client) = status.lock().await.deref() {
    ///         let filter_wheel = client.lock().await
    ///             .get_device("ASI EFW")
    ///             .await
    ///             .expect("Getting filter wheel");
    ///     };
    ///     
    /// };
    /// ```
    pub async fn get_device<'a>(
        &'a self,
        name: &str,
    ) -> Result<active_device::ActiveDevice, notify::Error<()>> {
        let subs = self.devices.subscribe().await;
        wait_fn(subs, Duration::from_secs(1), |devices| {
            if let Some(device) = devices.get(name) {
                return Ok(notify::Status::Complete(active_device::ActiveDevice::new(
                    String::from(name),
                    device.clone(),
                    self.feedback.clone(),
                )));
            }

            Ok(notify::Status::Pending)
        })
        .await
    }

    pub async fn device<'a, E>(&'a self, name: &str) -> Option<active_device::ActiveDevice> {
        self.devices.read().await.get(name).map(|device| {
            active_device::ActiveDevice::new(
                String::from(name),
                device.clone(),
                self.feedback.clone(),
            )
        })
    }

    /// Returns the a read-only lock on client's MemoryDeviceStore.
    pub fn get_devices(&self) -> Arc<Notify<MemoryDeviceStore>> {
        self.devices.clone()
    }

    pub fn get_connected(&self) -> Arc<Notify<bool>> {
        self.connected.clone()
    }

    pub fn join(&self) -> impl Future<Output = ()> {
        let connected = self.get_connected();
        async move {
            let mut connected = connected.subscribe().await;
            loop {
                match connected.next().await {
                    Some(Ok(connected)) => {
                        if !*connected {
                            break;
                        }
                    }
                    None | Some(Err(_)) => break,
                }
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.feedback.take();
        // self.devices = Arc::new(Notify::new(Default::default()));
    }
}

pub type MemoryDeviceStore = HashMap<String, Arc<Notify<device::Device<Notify<Parameter>>>>>;

pub trait DeviceStore {
    /// Update the state of the appropriate device property for a command that came from an INDI server.
    #[allow(async_fn_in_trait)]
    async fn update(
        &self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError>;

    #[allow(async_fn_in_trait)]
    async fn create(
        &mut self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError>;
}

impl DeviceStore for MemoryDeviceStore {
    async fn update(
        &self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                let device = self.get(name);
                match device {
                    Some(device) => Ok(Device::update(device.write().await, command).await?),
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    async fn create(
        &mut self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                dbg!("getting entry");
                let device = self
                    .entry(name.clone())
                    .or_insert(Arc::new(Notify::new(device::Device::new(name.clone()))));
                let device_lock = device.write().await;
                dbg!("got device_lock");
                Ok(Device::update(device_lock, command).await?)
                // Ok(None)
            }
            None => Ok(None),
        }
    }
}

pub trait AsyncClientConnection {
    type Reader: AsyncReadConnection + Unpin + MaybeSend + 'static;
    type Writer: AsyncWriteConnection + Unpin + MaybeSend + 'static;

    fn to_indi(self) -> (Self::Writer, Self::Reader);
}

pub trait AsyncReadConnection {
    fn read(
        &mut self,
    ) -> impl std::future::Future<Output = Option<Result<crate::Command, crate::DeError>>> + MaybeSend;
}

pub trait AsyncWriteConnection {
    fn write(
        &mut self,
        cmd: Command,
    ) -> impl std::future::Future<Output = Result<(), crate::DeError>> + MaybeSend;

    fn shutdown(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), crate::DeError>> + MaybeSend;
}

#[cfg(test)]
mod test {
    use futures::channel::oneshot;
    use tokio::net::{TcpListener, TcpStream};
    use tracing_test::traced_test;
    use twinkle_client::task::{Abortable, Task};

    use std::collections::HashSet;

    use super::*;

    #[tokio::test]
    #[traced_test]
    async fn test_client_updates() {
        tokio::time::timeout(Duration::from_secs(1), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let server_addr = listener.local_addr().unwrap();

            let (server_continue_tx, server_continue_rx) = oneshot::channel::<()>();
            // Server behavior
            tokio::spawn(async move {
                let (mut _socket, _) = listener.accept().await.unwrap();
                let (mut writer, mut reader) = _socket.to_indi();

                let msg = reader.read().await;
                info!("Got: {:?}", msg);
                server_continue_rx.await.expect("waiting to continue");
                info!("Sending commands");
                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device1".to_string(),
                            name: "name1".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::SetTextVector(
                        serialization::SetTextVector {
                            device: "device1".to_string(),
                            name: "name1".to_string(),
                            state: crate::PropertyState::Idle,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device1".to_string(),
                            name: "name2".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device2".to_string(),
                            name: "name1".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device1".to_string(),
                            name: Some("name2".to_string()),
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device1".to_string(),
                            name: Some("name1".to_string()),
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device1".to_string(),
                            name: None,
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device2".to_string(),
                            name: None,
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device3".to_string(),
                            name: "name1".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::SetTextVector(
                        serialization::SetTextVector {
                            device: "device3".to_string(),
                            name: "name1".to_string(),
                            state: crate::PropertyState::Idle,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device3".to_string(),
                            name: "name2".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::DefTextVector(
                        serialization::DefTextVector {
                            device: "device3".to_string(),
                            name: "name3".to_string(),
                            label: None,
                            group: Some("group1".to_string()),
                            state: crate::PropertyState::Idle,
                            perm: crate::PropertyPerm::RW,
                            timeout: None,
                            timestamp: None,
                            message: None,
                            texts: vec![],
                        },
                    ))
                    .await
                    .unwrap();

                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device3".to_string(),
                            name: Some("name1".to_string()),
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();
                writer
                    .write(serialization::Command::DelProperty(
                        serialization::DelProperty {
                            device: "device2".to_string(),
                            name: None,
                            timestamp: None,
                            message: None,
                        },
                    ))
                    .await
                    .unwrap();
                info!("Shutting down server");
            });

            let connection = TcpStream::connect(server_addr)
                .await
                .expect("connecting to indi");
            let client_task = new(connection, None, None).abort_on_drop(true);

            let _ = tokio::join!(
                client_task.with_state(|state| {
                    info!("building device_changes 1");
                    let client = state.clone();
                    async move {
                        info!("starting device_changes 1");
                        let lock = client.lock().await;
                        let mut devices = lock.get_devices().subscribe().await;
                        info!("subscribed to devices, continuing server");
                        let _ = server_continue_tx.send(());

                        let mut device_names_expected = vec![
                            vec![].into_iter().collect(),
                            vec!["device1".to_string()].into_iter().collect(),
                            vec!["device1".to_string(), "device2".to_string()]
                                .into_iter()
                                .collect(),
                            vec!["device2".to_string()].into_iter().collect(),
                            HashSet::new(),
                        ]
                        .into_iter();
                        loop {
                            let expected = match device_names_expected.next() {
                                Some(expected) => expected,
                                None => break,
                            };
                            match devices.next().await {
                                Some(Ok(devices)) => {
                                    let devices: HashSet<String> =
                                        devices.keys().map(Clone::clone).collect();
                                    info!("expected: {:?}", &expected);
                                    info!("devices : {:?}", devices);
                                    info!("****************************");
                                    assert_eq!(devices, expected);
                                }
                                _ => panic!("Not enough device changes"),
                            }
                        }
                        info!("finishing device_changes 1");
                    }
                }) // ,client_task.with_state(|state| {
                   //     info!("building device_changes 2");
                   //     let client = state.clone();
                   //     async move {
                   //         info!("starting device_changes 2");
                   //         let lock = client.lock().await;
                   //         let _devices = lock.get_device("device3").await.expect("Finding device3");
                   //         info!("finishing device_changes 2");
                   //     }})
            );
            info!("******************************************************");
            // todo!();
            // let device_changes = client_task.with_state(|state| {
            //     info!("building device_changes");
            //     let client = state.clone();
            //     async move {
            //         info!("starting device_changes");
            //         let mut devices = {
            //             let lock = client.lock().await;
            //             // lock.get_devices().subscribe().await
            //         };

            //     }
            // });
            // let device_task = client_task.with_state(|state| {
            //     info!("building device_changes");
            //     let client = state.clone();
            //     async move {
            //         info!("starting device_changes");
            //         let mut device3 = {
            //             let lock = client.lock().await;
            //             // lock
            //             //     .get_device("device3")
            //             //     .await
            //             //     .unwrap()
            //             //     .subscribe()
            //             //     .await
            //         };
            //         // info!("Got device3");

            //         // loop {
            //         //     match device3.next().await {
            //         //         Some(Ok(device)) => {
            //         //             let parameter_names: HashSet<String> =
            //         //                 device.get_parameters().keys().map(Clone::clone).collect();
            //         //             dbg!(parameter_names);
            //         //         }
            //         //         _ => break,
            //         //     }
            //         // }
            //         // dbg!("asldkjfalskjfalskdjf");
            //     }
            // });

            // tokio::time::sleep(Duration::from_millis(100)).await;
            // let _ = tokio::join!(
            //     async move { device_task.await.unwrap() },
            //     async move { device_changes.await.unwrap() },
            //     async move { server.await.unwrap() }
            // );
        })
        .await
        .expect("timeout");
    }
}
