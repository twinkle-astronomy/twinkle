use client::device;
use client::notify;
use client::notify::Notify;
use client::notify::Subscription;
use crossbeam_channel::Receiver;
use crossbeam_channel::Select;
use quick_xml::events;
use quick_xml::events::attributes::AttrError;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Result as XmlResult;
use quick_xml::{Reader, Writer};

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::net::TcpStream;

use std::num;
use std::num::Wrapping;

use std::ops::Deref;
use std::str;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use chrono::format::ParseError;
use chrono::prelude::*;
use std::io::Write;
use std::str::FromStr;

use std::collections::HashMap;

pub static INDI_PROTOCOL_VERSION: &str = "1.7";

pub mod serialization;
pub use serialization::*;

pub mod client;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PropertyState {
    Idle,
    Ok,
    Busy,
    Alert,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SwitchState {
    On,
    Off,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SwitchRule {
    OneOfMany,
    AtMostOne,
    AnyOfMany,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PropertyPerm {
    RO,
    WO,
    RW,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BlobEnable {
    Never,
    Also,
    Only,
}

pub trait FromParamValue {
    fn values_from(w: &Parameter) -> Result<&Self, TypeError>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq, Clone)]
pub struct Switch {
    pub label: Option<String>,
    pub value: SwitchState,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SwitchVector {
    pub gen: core::num::Wrapping<usize>,
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub rule: SwitchRule,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Switch>,
}

impl FromParamValue for HashMap<String, Switch> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::SwitchVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Number {
    pub label: Option<String>,
    pub format: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct NumberVector {
    pub gen: core::num::Wrapping<usize>,
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Number>,
}

impl FromParamValue for HashMap<String, Number> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::NumberVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Light {
    label: Option<String>,
    value: PropertyState,
}

#[derive(Debug, PartialEq, Clone)]
pub struct LightVector {
    pub gen: core::num::Wrapping<usize>,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Light>,
}

impl FromParamValue for HashMap<String, Light> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::LightVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Text {
    pub label: Option<String>,
    pub value: String,
}

impl FromParamValue for HashMap<String, Text> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::TextVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TextVector {
    pub gen: core::num::Wrapping<usize>,
    pub name: String,
    pub group: Option<String>,
    pub label: Option<String>,

    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Text>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Blob {
    pub label: Option<String>,
    pub format: Option<String>,
    pub value: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct BlobVector {
    pub gen: core::num::Wrapping<usize>,
    pub name: String,
    pub label: Option<String>,
    pub group: Option<String>,
    pub state: PropertyState,
    pub perm: PropertyPerm,
    pub timeout: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,

    pub values: HashMap<String, Blob>,
}

impl FromParamValue for HashMap<String, Blob> {
    fn values_from(p: &Parameter) -> Result<&Self, TypeError> {
        match p {
            Parameter::BlobVector(p) => Ok(&p.values),
            _ => Err(TypeError::TypeMismatch),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Parameter {
    TextVector(TextVector),
    NumberVector(NumberVector),
    SwitchVector(SwitchVector),
    LightVector(LightVector),
    BlobVector(BlobVector),
}

impl Parameter {
    pub fn get_group(&self) -> &Option<String> {
        match self {
            Parameter::TextVector(p) => &p.group,
            Parameter::NumberVector(p) => &p.group,
            Parameter::SwitchVector(p) => &p.group,
            Parameter::LightVector(p) => &p.group,
            Parameter::BlobVector(p) => &p.group,
        }
    }

    pub fn get_name(&self) -> &String {
        match self {
            Parameter::TextVector(p) => &p.name,
            Parameter::NumberVector(p) => &p.name,
            Parameter::SwitchVector(p) => &p.name,
            Parameter::LightVector(p) => &p.name,
            Parameter::BlobVector(p) => &p.name,
        }
    }
    pub fn get_label(&self) -> &Option<String> {
        match self {
            Parameter::TextVector(p) => &p.label,
            Parameter::NumberVector(p) => &p.label,
            Parameter::SwitchVector(p) => &p.label,
            Parameter::LightVector(p) => &p.label,
            Parameter::BlobVector(p) => &p.label,
        }
    }
    pub fn get_state(&self) -> &PropertyState {
        match self {
            Parameter::TextVector(p) => &p.state,
            Parameter::NumberVector(p) => &p.state,
            Parameter::SwitchVector(p) => &p.state,
            Parameter::LightVector(p) => &p.state,
            Parameter::BlobVector(p) => &p.state,
        }
    }
    pub fn get_timeout(&self) -> &Option<u32> {
        match self {
            Parameter::TextVector(p) => &p.timeout,
            Parameter::NumberVector(p) => &p.timeout,
            Parameter::SwitchVector(p) => &p.timeout,
            Parameter::LightVector(_) => &None,
            Parameter::BlobVector(p) => &p.timeout,
        }
    }

    pub fn get_values<T: FromParamValue>(&self) -> Result<&T, TypeError> {
        T::values_from(self)
    }

    pub fn gen(&self) -> core::num::Wrapping<usize> {
        match self {
            Parameter::TextVector(p) => p.gen,
            Parameter::NumberVector(p) => p.gen,
            Parameter::SwitchVector(p) => p.gen,
            Parameter::LightVector(p) => p.gen,
            Parameter::BlobVector(p) => p.gen,
        }
    }

    pub fn gen_mut(&mut self) -> &mut core::num::Wrapping<usize> {
        match self {
            Parameter::TextVector(p) => &mut p.gen,
            Parameter::NumberVector(p) => &mut p.gen,
            Parameter::SwitchVector(p) => &mut p.gen,
            Parameter::LightVector(p) => &mut p.gen,
            Parameter::BlobVector(p) => &mut p.gen,
        }
    }
}
#[derive(Debug)]
pub enum TypeError {
    TypeMismatch,
}
pub trait TryEq<T> {
    fn try_eq(&self, other: &T) -> Result<bool, TypeError>;
}

impl TryEq<Parameter> for Vec<OneSwitch> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Switch>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.value) == current_values.get(&other_value.name).map(|x| x.value)
        }))
    }
}

impl Into<SwitchState> for bool {
    fn into(self) -> SwitchState {
        match self {
            true => SwitchState::On,
            false => SwitchState::Off,
        }
    }
}
impl<I: Into<SwitchState> + Copy> TryEq<Parameter> for Vec<(&str, I)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Switch>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1.into()) == current_values.get(other_value.0).map(|x| x.value)
        }))
    }
}

impl TryEq<Parameter> for Vec<(&str, f64)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Number>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1) == current_values.get(other_value.0).map(|x| x.value)
        }))
    }
}

impl TryEq<Parameter> for Vec<OneNumber> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Number>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.value) == current_values.get(&other_value.name).map(|x| x.value)
        }))
    }
}

impl TryEq<Parameter> for Vec<(&str, &str)> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Text>>()?;

        Ok(self.iter().all(|other_value| {
            Some(other_value.1) == current_values.get(other_value.0).map(|x| x.value.as_str())
        }))
    }
}

impl TryEq<Parameter> for Vec<OneText> {
    fn try_eq(&self, other: &Parameter) -> Result<bool, TypeError> {
        let current_values = other.get_values::<HashMap<String, Text>>()?;

        Ok(self.iter().all(|other_value| {
            Some(&other_value.value) == current_values.get(&other_value.name).map(|x| &x.value)
        }))
    }
}

pub trait ToCommand<T> {
    fn to_command(self, device_name: String, param_name: String) -> Command;
}

impl<I: Into<SwitchState> + Copy> ToCommand<Vec<(&str, I)>> for Vec<(&str, I)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewSwitchVector(NewSwitchVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            switches: self
                .iter()
                .map(|x| OneSwitch {
                    name: String::from(x.0),
                    value: x.1.into(),
                })
                .collect(),
        })
    }
}

impl ToCommand<Vec<OneSwitch>> for Vec<OneSwitch> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewSwitchVector(NewSwitchVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            switches: self,
        })
    }
}

impl ToCommand<Vec<(&str, f64)>> for Vec<(&str, f64)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewNumberVector(NewNumberVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            numbers: self
                .iter()
                .map(|x| OneNumber {
                    name: String::from(x.0),
                    value: x.1,
                })
                .collect(),
        })
    }
}
impl ToCommand<Vec<OneNumber>> for Vec<OneNumber> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewNumberVector(NewNumberVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            numbers: self,
        })
    }
}

impl ToCommand<Vec<(&str, &str)>> for Vec<(&str, &str)> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewTextVector(NewTextVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            texts: self
                .iter()
                .map(|x| OneText {
                    name: String::from(x.0),
                    value: String::from(x.1),
                })
                .collect(),
        })
    }
}
impl ToCommand<Vec<OneText>> for Vec<OneText> {
    fn to_command(self, device_name: String, param_name: String) -> Command {
        Command::NewTextVector(NewTextVector {
            device: device_name,
            name: param_name,
            timestamp: Some(chrono::offset::Utc::now()),
            texts: self,
        })
    }
}
#[derive(Debug, PartialEq)]
pub enum UpdateError {
    ParameterMissing(String),
    ParameterTypeMismatch(String),
}

pub enum Action {
    Define,
    Update,
    Delete,
}

pub trait CommandtoParam {
    fn get_name(&self) -> &String;
    fn get_group(&self) -> &Option<String>;
    fn to_param(self, gen: Wrapping<usize>) -> Parameter;
}

pub trait CommandToUpdate {
    fn get_name(&self) -> &String;
    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError>;
}

pub enum ClientErrors {
    DeError(DeError),
    UpdateError(UpdateError),
}

impl From<DeError> for ClientErrors {
    fn from(err: DeError) -> Self {
        ClientErrors::DeError(err)
    }
}
impl From<UpdateError> for ClientErrors {
    fn from(err: UpdateError) -> Self {
        ClientErrors::UpdateError(err)
    }
}

/// Struct used to keep track of a the devices and their properties.
/// When used in conjunction with the Connection struct can be used to
/// track and control devices managed by an INDI server.
pub struct Client<T: ClientConnection + 'static> {
    pub devices: Arc<Notify<MemoryDeviceStore>>,
    pub connection: T,
    feedback: crossbeam_channel::Sender<Command>,
}

#[derive(Debug)]
pub enum ChangeError<E> {
    NotifyError(notify::Error<E>),
    DeError(serialization::DeError),
    IoError(std::io::Error),
    Disconnected(crossbeam_channel::SendError<Command>),
    DisconnectedRecv(crossbeam_channel::TryRecvError),
    DisconnectedRecvTimeout(crossbeam_channel::RecvTimeoutError),
    Abort,
    PropertyError,
    TypeMismatch,
}

impl<T> From<crossbeam_channel::RecvTimeoutError> for ChangeError<T> {
    fn from(value: crossbeam_channel::RecvTimeoutError) -> Self {
        ChangeError::DisconnectedRecvTimeout(value)
    }
}
impl From<notify::Error<ChangeError<serialization::Command>>> for ChangeError<Command> {
    fn from(value: notify::Error<ChangeError<serialization::Command>>) -> Self {
        match value {
            notify::Error::Timeout => ChangeError::Abort,
            notify::Error::Canceled => ChangeError::Abort,
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

impl<E> From<crossbeam_channel::TryRecvError> for ChangeError<E> {
    fn from(value: crossbeam_channel::TryRecvError) -> Self {
        ChangeError::DisconnectedRecv(value)
    }
}

pub trait PendingChange {
    // fn cancel(&self);
    fn wait(&self) -> Result<Arc<Notify<Parameter>>, ChangeError<Command>>;
    fn remaining(&self) -> Duration;
    fn deadline(&self) -> Instant;
    fn receiver(&self) -> &Receiver<Arc<Parameter>>;
    fn tick(&self, item: Arc<Parameter>) -> Result<notify::Status<Arc<Parameter>>, ChangeError<Command>>;
    fn abort(&self);
}

pub struct PendingChangeImpl<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static> {
    subscription: Subscription<Parameter>,
    param: Arc<Notify<Parameter>>,
    deadline: Instant,
    values: P,
}
// asdfadsf
impl<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static> PendingChange for PendingChangeImpl<P> {
    fn wait(&self) -> Result<Arc<Notify<Parameter>>, ChangeError<Command>> {
        let r = self
            .subscription
            .wait_fn::<Arc<Notify<Parameter>>, ChangeError<Command>, _>(
                self.remaining(),
                |param_lock| {
                    if *param_lock.get_state() == PropertyState::Alert {
                        return Err(ChangeError::PropertyError);
                    }
                    if self.values.try_eq(&param_lock)? {
                        Ok(notify::Status::Complete(self.param.clone()))
                    } else {
                        Ok(notify::Status::Pending)
                    }
                },
            )?;
        Ok(r)
    }

    fn tick(&self, next: Arc<Parameter>) -> Result<notify::Status<Arc<Parameter>>, ChangeError<Command>> {
        if *next.get_state() == PropertyState::Alert {
            return Err(ChangeError::PropertyError);
        }
        if self.values.try_eq(&next)? {
            Ok(notify::Status::Complete(next.clone()))
        } else {
            Ok(notify::Status::Pending)
        }
    }

    fn remaining(&self) -> Duration {
        self.deadline.duration_since(Instant::now())
    }

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn receiver(&self) -> &Receiver<Arc<Parameter>> {
        self.subscription.deref()
    }

    fn abort(&self) {
        self.param.cancel(&self.subscription);
    }
}

pub struct PendingChangeBatch {
    changes: Vec<Box<dyn PendingChange>>,
}

impl PendingChangeBatch {
    pub fn new() -> PendingChangeBatch {
        PendingChangeBatch {
            changes: Default::default(),
        }
    }

    pub fn add<T: PendingChange + 'static>(mut self, pending_change: T) -> PendingChangeBatch {
        self.changes.push(Box::new(pending_change));
        self
    }

    pub fn wait(self) -> Result<(), ChangeError<Command>> {
        let mut sel = Select::new();
        let mut remaining = BTreeSet::new();
        for (i, r) in self.changes.iter().enumerate() {
            sel.recv(r.receiver());
            remaining.insert(i);
        }

        loop {
            let selected = sel.select();
            let i = selected.index();
            let r = selected.recv(self.changes[i].receiver()).unwrap();
            match self.changes[i].tick(r) {
                Ok(v) => {
                    if let notify::Status::Complete(_v) = v {
                        remaining.remove(&i);
                    }
                    if remaining.is_empty() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    for i in &remaining {
                        self.changes[*i].abort();
                    }
                    return Err(e);
                }
            }
        }
    }
}

pub fn batch<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static>(
    changes: Vec<PendingChangeImpl<P>>,
) -> Result<(), crate::ChangeError<Command>> {
    let mut batch = PendingChangeBatch::new();

    for f in changes {
        batch = batch.add(f);
    }

    batch.wait()
}

impl<T: ClientConnection> Client<T> {
    /// Create a new client object.
    pub fn new(
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
            let mut xml_writer =
                Writer::new_with_indent(BufWriter::new(thread_connection), b' ', 2);
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
                        let mut locked_devices = thread_devices.lock();
                        let update_result = locked_devices.update(command, |_param| {});
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
        Ok(c)
    }

    pub fn get_device<'a>(
        &'a self,
        name: &str,
    ) -> Result<client::device::ActiveDevice, notify::Error<()>> {
        self.devices
            .subscribe()
            .wait_fn::<_, (), _>(Duration::from_secs(60), |devices| {
                if let Some(device) = devices.get(name) {
                    return Ok(notify::Status::Complete(device::ActiveDevice::new(
                        device.clone(),
                        self.feedback.clone(),
                    )));
                }

                Ok(notify::Status::Pending)
            })
    }
}

pub type MemoryDeviceStore = HashMap<String, Arc<Notify<client::device::Device>>>;

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
                    .or_insert(Arc::new(Notify::new(client::device::Device::new(
                        name.clone(),
                    ))))
                    .lock();
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
    /// use crate::indi::{ClientConnection, DeviceStore};
    /// use crate::indi::client::device::Device;
    /// use std::net::TcpStream;
    /// let mut connection = TcpStream::connect("localhost:7624").unwrap();
    /// connection.write(&indi::GetProperties {
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
    /// use crate::indi::ClientConnection;
    /// use std::net::TcpStream;
    /// let mut connection = TcpStream::connect("localhost:7624").unwrap();
    /// connection.write(&indi::GetProperties {
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
