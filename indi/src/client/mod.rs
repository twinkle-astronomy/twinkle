pub mod device;
pub mod notify;

use std::{
    collections::HashMap,
    io::{BufReader, BufWriter, Write},
    net::TcpStream,
    sync::{Arc, MutexGuard, PoisonError},
    thread,
    time::Duration,
};

use quick_xml::{Reader, Writer};

use crate::{
    serialization, Command, DeError, GetProperties, Parameter, TypeError, UpdateError,
    XmlSerialization, INDI_PROTOCOL_VERSION,
};

use self::notify::{wait_fn, Notify, NotifyMutexGuard};

#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
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
            notify::Error::PoisonError => ChangeError::PoisonError,
            notify::Error::Abort(e) => e,
        }
    }
}

impl<E> From<std::io::Error> for ChangeError<E> {
    fn from(value: std::io::Error) -> Self {
        ChangeError::<E>::IoError(value)
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
/// use std::net::TcpStream;
/// use crate::indi::client::ClientConnection;
/// // Client that will track all devices and parameters to the connected INDI server at localhost.
/// let client = indi::client::new(TcpStream::connect("localhost:7624").expect("Connecting to server"), None, None).expect("Initializing connection to INDI server");
///
/// // Client that will only track the blob parameter for an image.  It is recommended to use a dedicated
/// //  client connection for retreiving images, as other indi updates will be delayed when images are being transfered.
/// let image_client = indi::client::new(
///     TcpStream::connect("localhost:7624").expect("Connecting to server"),
///     Some("ZWO CCD ASI294MM Pro"),
///     Some("CCD1"),
/// ).expect("Connecting to INDI server");
/// async {
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
pub fn new<T: ClientConnection>(
    connection: T,
    device: Option<&str>,
    parameter: Option<&str>,
) -> Result<Client<T>, std::io::Error> {
    connection
        .write(&GetProperties {
            version: INDI_PROTOCOL_VERSION.to_string(),
            device: device.map(|x| String::from(x)),
            name: parameter.map(|x| String::from(x)),
        })
        .expect("Unable to write command");
    let (feedback, incoming_commands) = crossbeam_channel::unbounded();
    let c = Client {
        devices: Arc::new(Notify::new(HashMap::new())),
        connection,
        feedback,
    };

    let thread_connection = c.connection.clone_writer()?;
    thread::spawn(move || {
        let mut xml_writer = Writer::new_with_indent(BufWriter::new(thread_connection), b' ', 2);
        for command in incoming_commands.iter() {
            command
                .write(&mut xml_writer)
                .expect("Writing command to connection");
            xml_writer.inner().flush().expect("Flushing connection");
        }
    });

    let thread_devices = c.devices.clone();
    let connection_iter = c.connection.iter()?;
    thread::spawn(move || {
        for command in connection_iter {
            match command {
                Ok(command) => {
                    let locked_devices = thread_devices.lock();
                    match locked_devices {
                        Ok(mut locked_devices) => {
                            let update_result = locked_devices.update(command, |_param| {});
                            if let Err(e) = update_result {
                                dbg!(e);
                            }
                        }
                        Err(e) => {
                            dbg!(e);
                        }
                    }
                }
                Err(e) => {
                    dbg!(&e);
                }
            }
        }
    });
    Ok(c)
}

/// Struct used to keep track of a the devices and their properties.
pub struct Client<T: ClientConnection + 'static> {
    devices: Arc<Notify<MemoryDeviceStore>>,
    connection: T,
    feedback: crossbeam_channel::Sender<Command>,
}

impl<T: ClientConnection> Client<T> {
    /// Async method that will wait up to 1 second for the device named `name` to be defined
    ///  by the INDI server.  The returned `ActiveDevice` (if present) will be associated with
    ///  the `self` client for communicating changes with the INDI server it came from.
    ///
    /// # Arguments
    /// * `name` - Name of device on the remote INDI server you wish to get.
    ///
    /// # Example
    /// ```no_run
    /// use std::net::TcpStream;
    /// // Client that will track all devices and parameters to the connected INDI server at localhost.
    /// let client = indi::client::new(TcpStream::connect("localhost:7624").expect("Connecting to server"), None, None).expect("Initializing connection to INDI server");
    /// async {
    /// let filter_wheel = client
    ///     .get_device::<()>("ASI EFW")
    ///     .await
    ///     .expect("Getting filter wheel");
    ///
    /// };
    /// ```
    pub async fn get_device<'a, E>(
        &'a self,
        name: &str,
    ) -> Result<device::ActiveDevice, notify::Error<E>> {
        let subs = self.devices.subscribe()?;
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
    pub fn get_devices(
        &self,
    ) -> Result<NotifyMutexGuard<MemoryDeviceStore>, PoisonError<MutexGuard<Arc<MemoryDeviceStore>>>>
    {
        self.devices.lock()
    }
}

pub type MemoryDeviceStore = HashMap<String, Arc<Notify<device::Device>>>;

pub trait DeviceStore {
    /// Update the state of the appropriate device property for a command that came from an INDI server.
    fn update<T>(
        &mut self,
        command: serialization::Command,
        f: impl FnOnce(notify::NotifyMutexGuard<Parameter>) -> T,
    ) -> Result<Option<T>, UpdateError>;
}

impl DeviceStore for MemoryDeviceStore {
    fn update<T>(
        &mut self,
        command: serialization::Command,
        f: impl FnOnce(notify::NotifyMutexGuard<Parameter>) -> T,
    ) -> Result<Option<T>, UpdateError> {
        let name = command.device_name();
        match name {
            Some(name) => {
                let mut device = self
                    .entry(name.clone())
                    .or_insert(Arc::new(Notify::new(device::Device::new(name.clone()))))
                    .lock()?;
                let param = device.update(command)?;
                Ok(match param {
                    Some(p) => Some(f(p)),
                    None => None,
                })
            }
            None => Ok(None),
        }
    }
}

pub trait ClientConnection {
    type Read: std::io::Read + Send;
    type Write: std::io::Write + Send;

    /// Creates an interator that yields commands from the the connected INDI server.
    /// Example usage:
    /// ```no_run
    /// use std::collections::HashMap;
    /// use crate::indi::client::{ClientConnection, DeviceStore};
    /// use crate::indi::client::device::Device;
    /// use std::net::TcpStream;
    /// let mut connection = TcpStream::connect("localhost:7624").unwrap();
    /// connection.write(&indi::serialization::GetProperties {
    ///     version: indi::INDI_PROTOCOL_VERSION.to_string(),
    ///     device: None,
    ///     name: None,
    /// }).unwrap();
    ///
    /// let mut client = HashMap::<String, Device>::new();
    ///
    /// for command in connection.iter().unwrap() {
    ///     println!("Command: {:?}", command);
    /// }
    fn iter(&self) -> Result<serialization::CommandIter<BufReader<Self::Read>>, std::io::Error> {
        let mut xml_reader = Reader::from_reader(BufReader::new(self.clone_reader()?));

        xml_reader.trim_text(true);
        xml_reader.expand_empty_elements(true);

        let iter = serialization::CommandIter::new(xml_reader);
        Ok(iter)
    }

    /// Sends the given INDI command to the connected server.  Consumes the command.
    /// Example usage:
    /// ```no_run
    /// use crate::indi::client::ClientConnection;
    /// use std::net::TcpStream;
    /// let mut connection = TcpStream::connect("localhost:7624").unwrap();
    /// connection.write(&indi::serialization::GetProperties {
    ///     version: indi::INDI_PROTOCOL_VERSION.to_string(),
    ///     device: None,
    ///     name: None,
    /// }).unwrap();
    ///
    fn write<X: XmlSerialization>(&self, command: &X) -> Result<(), DeError>
    where
        <Self as ClientConnection>::Write: std::io::Write,
    {
        let mut xml_writer = Writer::new_with_indent(BufWriter::new(self.clone_writer()?), b' ', 2);

        command.write(&mut xml_writer)?;
        xml_writer.inner().flush()?;
        Ok(())
    }

    fn clone_reader(&self) -> Result<Self::Read, std::io::Error>;
    fn clone_writer(&self) -> Result<Self::Write, std::io::Error>;
}

impl ClientConnection for TcpStream {
    type Read = TcpStream;
    type Write = TcpStream;

    fn clone_reader(&self) -> Result<TcpStream, std::io::Error> {
        self.try_clone()
    }
    fn clone_writer(&self) -> Result<TcpStream, std::io::Error> {
        self.try_clone()
    }
}
