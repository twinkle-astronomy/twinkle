pub mod device;
pub mod tcpstream;
pub mod websocket;

use twinkle_client;

use std::{
    collections::HashMap,
    sync::{Arc, PoisonError},
    time::Duration,
};

use self::device::ParamUpdateResult;
use crate::{
    serialization, Command, DeError, GetProperties, TypeError, UpdateError, INDI_PROTOCOL_VERSION,
};
pub use twinkle_client::notify::{self, wait_fn, Notify};

#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
    SendError(device::SendError<Command>),
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
impl<E> From<device::SendError<Command>> for ChangeError<E> {
    fn from(value: device::SendError<Command>) -> Self {
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
pub fn new<T: AsyncClientConnection>(
    connection: T,
    device: Option<&str>,
    parameter: Option<&str>,
) -> Result<Client, serialization::DeError> {
    let (feedback, mut incoming_commands) = tokio::sync::mpsc::unbounded_channel::<Command>();

    let (mut writer, mut reader) = connection.to_indi();
    let writer_device = device.map(|x| String::from(x));
    let writer_parameter = parameter.map(|x| String::from(x));
    let writer_thread = tokio::task::spawn(async move {
        writer
            .write(serialization::Command::GetProperties(GetProperties {
                version: INDI_PROTOCOL_VERSION.to_string(),
                device: writer_device,
                name: writer_parameter,
            }))
            .await?;

        loop {
            let command = match incoming_commands.recv().await {
                Some(c) => c,
                None => break,
            };
            writer.write(command).await?;
        }
        writer.shutdown().await?;
        Ok(())
    });
    let devices = Arc::new(Notify::new(HashMap::new()));
    let thread_devices = devices.clone();
    let reader_thread = tokio::spawn(async move {
        loop {
            let command = match reader.read().await {
                Some(c) => c,
                None => break,
            };
            match command {
                Ok(command) => {
                    let mut locked_devices = thread_devices.lock().await;

                    let update_result = locked_devices.update(command, |_param| {}).await;
                    if let Err(e) = update_result {
                        dbg!(e);
                    }
                }
                Err(e) => {
                    dbg!(&e);
                }
            }
        }
    });
    let c = Client {
        devices,
        feedback: Some(feedback),
        _workers: Some((writer_thread, reader_thread)),
    };
    Ok(c)
}

/// Struct used to keep track of a the devices and their properties.
pub struct Client {
    devices: Arc<Notify<MemoryDeviceStore>>,
    feedback: Option<tokio::sync::mpsc::UnboundedSender<Command>>,
    // connection: T,
    // Used for testing
    _workers: Option<(
        tokio::task::JoinHandle<Result<(), DeError>>,
        tokio::task::JoinHandle<()>,
    )>,
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
    ) -> Result<device::ActiveDevice, notify::Error<E>> {
        let subs = self.devices.subscribe().await;
        wait_fn(subs, Duration::from_secs(1), |devices| {
            if let Some(device) = devices.get(name) {
                return Ok(notify::Status::Complete(device::ActiveDevice::new(
                    String::from(name),
                    device.clone(),
                    self.feedback.clone(),
                )));
            }

            Ok(notify::Status::Pending)
        })
        .await
    }

    /// Returns the a read-only lock on client's MemoryDeviceStore.
    pub fn get_devices(&self) -> Arc<Notify<MemoryDeviceStore>> {
        self.devices.clone()
    }

    pub fn shutdown(&mut self) {
        self.feedback.take();
    }
}

pub type MemoryDeviceStore = HashMap<String, Arc<Notify<device::Device>>>;

pub trait DeviceStore {
    /// Update the state of the appropriate device property for a command that came from an INDI server.
    #[allow(async_fn_in_trait)]
    async fn update<T>(
        &mut self,
        command: serialization::Command,
        f: impl FnOnce(ParamUpdateResult) -> T,
    ) -> Result<Option<T>, UpdateError>;
}

impl DeviceStore for MemoryDeviceStore {
    async fn update<T>(
        &mut self,
        command: serialization::Command,
        f: impl FnOnce(ParamUpdateResult) -> T,
    ) -> Result<Option<T>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                let mut device = self
                    .entry(name.clone())
                    .or_insert(Arc::new(Notify::new(device::Device::new(name.clone()))))
                    .lock()
                    .await;
                let param = device.update(command).await?;
                Ok(Some(f(param)))
            }
            None => Ok(None),
        }
    }
}

pub trait AsyncClientConnection {
    type Reader: AsyncReadConnection + Unpin + Send + 'static;
    type Writer: AsyncWriteConnection + Unpin + Send + 'static;

    fn to_indi(self) -> (Self::Writer, Self::Reader);
}

pub trait AsyncReadConnection {
    fn read(
        &mut self,
    ) -> impl std::future::Future<Output = Option<Result<crate::Command, crate::DeError>>> + Send;
}

pub trait AsyncWriteConnection {
    fn write(
        &mut self,
        cmd: Command,
    ) -> impl std::future::Future<Output = Result<(), crate::DeError>> + Send;

    fn shutdown(&mut self) -> impl std::future::Future<Output = Result<(), crate::DeError>> + Send;
}
