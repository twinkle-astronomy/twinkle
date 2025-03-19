pub mod active_device;
pub mod active_parameter;
pub mod sink;
pub mod stream;
#[cfg(not(target_arch = "wasm32"))]
pub mod tcpstream;
pub mod websocket;

use crate::{device, serialization::device::DeviceUpdate};
use std::{fmt::Debug, future::Future};
use tokio::{select, sync::oneshot};
use tokio_stream::StreamExt;
use tracing::{error, info, Instrument};
use twinkle_client;

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

#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}

// Helper trait that requires Send for non-wasm
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send> MaybeSend for T {}

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
/// # Examples
/// ```no_run
/// use tokio::net::TcpStream;
/// // Client that will track all devices and parameters to the connected INDI server at localhost.
/// async {
///     let client = indi::client::new(TcpStream::connect("localhost:7624").await.expect("Connecting to server"), None, None).expect("Initializing connection to INDI server");
///
///     // Client that will only track the blob parameter for an image.  It is recommended to use a dedicated
///     //  client connection for retreiving images, as other indi updates will be delayed when images are being transfered.
///     let image_client = indi::client::new(
///         TcpStream::connect("localhost:7624").await.expect("Connecting to server"),
///         Some("ZWO CCD ASI294MM Pro"),
///         Some("CCD1"),
///     ).expect("Connecting to INDI server");
///     // Retrieve the camera and set BlobEnable to `Only` to ensure this connection
///     //  is only used for transfering images.
///     let image_camera = image_client
///         .get_device::<()>("ZWO CCD ASI294MM Pro")
///         .await
///         .expect("Getting imaging camera");
///     image_camera
///         .enable_blob(Some("CCD1"), indi::BlobEnable::Only)
///         .await
///         .expect("enabling image retrieval");
/// };
/// ```
/// 
pub fn new<T: AsyncClientConnection>(
    connection: T,
    device: Option<&str>,
    parameter: Option<&str>,
) -> Result<Client, serialization::DeError> {
    let (writer, reader) = connection.to_indi();
    new_with_streams(writer, reader, device, parameter)
}

pub fn new_with_streams(
    mut writer: impl AsyncWriteConnection + MaybeSend + 'static,
    mut reader: impl AsyncReadConnection + MaybeSend + 'static,
    device: Option<&str>,
    parameter: Option<&str>,
) -> Result<Client, serialization::DeError> {
    let connected = Arc::new(Notify::new(true));
    let (feedback, mut incoming_commands) = tokio::sync::mpsc::unbounded_channel::<Command>();

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
        };

        select! {
            s = sender => {
                if let Err(e) = s {
                    error!("Error in sending task: {:?}", e);
                }
            }
            _ = reader_finished_rx => { }
        }

        if let Err(e) = writer.shutdown().await {
            error!("Error shutting down writer: {:?}", e);
        }
        {
            *writer_connected.lock().await = false;
        }
    }
    .instrument(tracing::info_span!("indi_writer"));
    let devices = Arc::new(Notify::new(HashMap::new()));
    let thread_devices = devices.clone();
    let reader_future = async move {
        loop {
            let command = match reader.read().await {
                Some(c) => c,
                None => break,
            };
            match command {
                Ok(command) => {
                    let mut locked_devices = thread_devices.lock().await;

                    let update_result = locked_devices.update(command).await;
                    if let Err(e) = update_result {
                        error!("Device update error: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Command error: {:?}", e);
                }
            }
        }
        *thread_devices.lock().await = Default::default();
        let _ = reader_finished_tx.send(());
    }
    .instrument(tracing::info_span!("indi_reader"));

    #[cfg(not(target_arch = "wasm32"))]
    let c = {
        let (writer_thread, reader_thread) =
            { (tokio::spawn(writer_future), tokio::spawn(reader_future)) };
        Client {
            devices,
            connected,
            feedback: Some(feedback),
            _workers: Some((writer_thread, reader_thread)),
        }
    };
    #[cfg(target_arch = "wasm32")]
    let c = {
        wasm_bindgen_futures::spawn_local(writer_future);
        wasm_bindgen_futures::spawn_local(reader_future);
        Client {
            devices,
            connected,
            feedback: Some(feedback),
        }
    };
    Ok(c)
}

/// Struct used to keep track of a the devices and their properties.
pub struct Client {
    devices: Arc<Notify<MemoryDeviceStore>>,
    connected: Arc<Notify<bool>>,
    feedback: Option<tokio::sync::mpsc::UnboundedSender<Command>>,
    // connection: T,
    // Used for testing
    #[cfg(not(target_arch = "wasm32"))]
    _workers: Option<(tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>)>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Client {
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
    /// // Client that will track all devices and parameters to the connected INDI server at localhost.
    ///
    /// async {
    ///     let client = indi::client::new(TcpStream::connect("localhost:7624").await.expect("Connecting to server"), None, None).expect("Initializing connection to INDI server");
    ///     let filter_wheel = client
    ///         .get_device::<()>("ASI EFW")
    ///         .await
    ///         .expect("Getting filter wheel");
    /// };
    /// ```
    pub async fn get_device<'a, E>(
        &'a self,
        name: &str,
    ) -> Result<active_device::ActiveDevice, notify::Error<E>> {
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
        self.devices.lock().await.get(name).map(|device| {
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

    pub fn join(&self) -> impl Future<Output=()> {
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
                    None | Some(Err(_)) => {
                        break
                    }
                }
            }
        }
    }

    pub fn command_sender(&self) -> Option<tokio::sync::mpsc::UnboundedSender<Command>>{
        self.feedback.clone()
    }

    pub fn shutdown(&mut self) {
        self.feedback.take();
        self.devices = Arc::new(Notify::new(Default::default()));
    }
}

pub type MemoryDeviceStore = HashMap<String, Arc<Notify<device::Device<Notify<Parameter>>>>>;

pub trait DeviceStore {
    /// Update the state of the appropriate device property for a command that came from an INDI server.
    #[allow(async_fn_in_trait)]
    async fn update(&mut self, command: serialization::Command) -> Result<Option<DeviceUpdate>, UpdateError>;
}

impl DeviceStore for MemoryDeviceStore {
    async fn update(&mut self, command: serialization::Command) -> Result<Option<DeviceUpdate>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                let device = self
                    .entry(name.clone())
                    .or_insert(Arc::new(Notify::new(device::Device::new(name.clone()))));
                let mut device_guard = device.lock().await;
                Ok(device_guard.update(command).await?)
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
