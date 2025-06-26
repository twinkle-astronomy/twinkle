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
use std::fmt::Debug;
use tokio::sync::{
    mpsc::{error::SendError, UnboundedReceiver},
    oneshot,
};
use tracing::{error, Instrument};
use twinkle_client::{self, MaybeSend};

use std::{
    collections::HashMap,
    sync::{Arc, PoisonError},
    time::Duration,
};

use crate::{
    serialization, Command, DeError, TypeError, UpdateError,
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

pub async fn start<T: AsyncClientConnection>(
    client: MemoryDeviceStore,
    incoming_commands: UnboundedReceiver<Command>,
    connection: T,
) {
    let (writer, reader) = connection.to_indi();
    start_with_streams(client, incoming_commands, writer, reader).await
}

pub async fn start_with_streams(
    devices: MemoryDeviceStore,
    mut incoming_commands: UnboundedReceiver<Command>,
    mut writer: impl AsyncWriteConnection + MaybeSend + 'static,
    mut reader: impl AsyncReadConnection + MaybeSend + 'static,
) {
    let (reader_finished_tx, reader_finished_rx) = oneshot::channel::<()>();

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

        if let Err(e) = writer.shutdown().await {
            error!("Error shutting down writer: {:?}", e);
        }
        let _ = reader_finished_rx.await;
    }
    .instrument(tracing::info_span!("indi_writer"));
    let thread_devices = devices.clone();
    let reader_future = async move {
        loop {
            let command = match reader.read().await {
                Some(c) => c,
                None => break,
            };
            match command {
                Ok(command) => {
                    let update_result = DeviceStore::update(&thread_devices, command).await;
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
    }
    .instrument(tracing::info_span!("indi_reader"));

    tokio::select! {
        _ = writer_future => tracing::info!("writer_future finisehd"),
        _ = reader_future => tracing::info!("reader_future finisehd"),
    }
}

/// Struct used to keep track of a the devices and their properties.
#[derive(Clone)]
pub struct Client {
    devices: MemoryDeviceStore,
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
            feedback,
        }
    }

    pub async fn reset(&mut self, feedback: Option<tokio::sync::mpsc::UnboundedSender<Command>>) {
        let mut lock = self.devices.write().await;
        lock.clear();
        self.feedback = feedback;
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
        let mut subs = self.devices.subscribe().await;
        wait_fn(&mut subs, Duration::from_secs(10), |devices| {
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
    pub fn get_devices(&self) -> &MemoryDeviceStore {
        &self.devices
    }

    pub fn shutdown(&mut self) {
        self.feedback.take();
    }

    pub fn send(&self, cmd: Command) -> Result<(), SendError<Command>> {
        if let Some(feedback) = &self.feedback {
            feedback.send(cmd)?;
        }
        Ok(())
    }
}

pub type MemoryDeviceStore = Arc<Notify<HashMap<String, Arc<Notify<device::Device>>>>>;

pub trait DeviceStore {
    /// Update the state of the appropriate device property for a command that came from an INDI server.
    #[allow(async_fn_in_trait)]
    async fn update(
        &self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError>;
}

impl DeviceStore for MemoryDeviceStore {
    async fn update(
        &self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        let locked_devices = self.write().await;
        match command.param_update_type() {
            serialization::ParamUpdateType::Add => {
                let device_name = command.device_name().cloned();
                if let Some(device_name) = device_name {
                    if locked_devices.contains_key(&device_name) {
                        update(&locked_devices, command).await
                    } else {
                        let mut locked_devices = locked_devices;
                        create(&mut locked_devices, command).await
                    }
                } else {
                    Ok(None)
                }
            }
            serialization::ParamUpdateType::Update => update(&locked_devices, command).await,
            serialization::ParamUpdateType::Remove => {
                let device_name = command.device_name().cloned();

                let update = update(&locked_devices, command).await;
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
        }
    }
}

async fn update(
    devices: &HashMap<String, Arc<Notify<device::Device>>>,
    command: serialization::Command,
) -> Result<Option<DeviceUpdate>, UpdateError> {
    let name = command.device_name();
    match name {
        Some(name) => {
            let device = devices.get(name);
            match device {
                Some(device) => Ok(Device::update(device.write().await, command).await?),
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}

async fn create(
    devices: &mut HashMap<String, Arc<Notify<device::Device>>>,
    command: serialization::Command,
) -> Result<Option<DeviceUpdate>, UpdateError> {
    let name = command.device_name();
    match name {
        Some(name) => {
            let device = devices
                .entry(name.clone())
                .or_insert(Arc::new(Notify::new(device::Device::new(name.clone()))));
            let device_lock = device.write().await;
            Ok(Device::update(device_lock, command).await?)
        }
        None => Ok(None),
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
            let task = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let (mut writer, mut reader) = socket.to_indi();
                let _msg = reader.read().await;

                server_continue_rx.await.expect("waiting to continue");
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
            });

            let connection = TcpStream::connect(server_addr)
                .await
                .expect("connecting to indi");

            let (_client_task, client) = new(connection, None, None);
            let mut devices = client.get_devices().subscribe().await;
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

            while let Some(expected) = device_names_expected.next() {
                match devices.next().await {
                    Some(Ok(devices)) => {
                        let devices: HashSet<String> = devices.keys().map(Clone::clone).collect();
                        assert_eq!(devices, expected);
                    }
                    _ => panic!("Not enough device changes"),
                }
            }
            task.await.unwrap();
        })
        .await
        .expect("timeout");
    }
}
