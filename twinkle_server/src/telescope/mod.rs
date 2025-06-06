use std::{fmt::Display, sync::Arc};

use camera::Camera;
use filter_wheel::FilterWheel;
use flat_panel::FlatPanel;
use indi::{
    client::{
        active_device::{ActiveDevice, SendError},
        ChangeError, Client,
    },
    serialization::{self, Command, GetProperties},
    Parameter, TypeError, INDI_PROTOCOL_VERSION,
};
use tokio::net::TcpStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::Stream;
use twinkle_api::settings::{Settings, TelescopeConfig};
use twinkle_client::notify::Notify;
use twinkle_client::{
    agent::Agent,
    notify::{self, ArcCounter},
    task::{Abortable, Joinable, TaskStatusError},
};

pub mod camera;
pub mod filter_wheel;
pub mod flat_panel;
mod parameter_with_config;

#[derive(Debug)]
pub enum DeviceSelectionError {
    DeviceMismatch,
    DeviceError(DeviceError),
    NotifyError(twinkle_client::notify::Error<()>),
    UnkownDevice,
}

impl From<twinkle_client::notify::Error<()>> for DeviceSelectionError {
    fn from(value: twinkle_client::notify::Error<()>) -> Self {
        DeviceSelectionError::NotifyError(value)
    }
}

impl From<DeviceError> for DeviceSelectionError {
    fn from(value: DeviceError) -> Self {
        DeviceSelectionError::DeviceError(value)
    }
}

#[derive(Debug)]
pub enum TelescopeError<E> {
    NotifyError(notify::Error<E>),
    ChangeError(ChangeError<()>),
    DeviceSelectionError(DeviceSelectionError),
    Disconnected(twinkle_client::task::Error),
    DeviceError(DeviceError),
    TaskStatusError(TaskStatusError),
}

impl<E> From<TaskStatusError> for TelescopeError<E> {
    fn from(value: TaskStatusError) -> Self {
        TelescopeError::TaskStatusError(value)
    }
}

impl<E> From<twinkle_client::task::Error> for TelescopeError<E> {
    fn from(v: twinkle_client::task::Error) -> Self {
        TelescopeError::Disconnected(v)
    }
}

impl<E> From<notify::Error<E>> for TelescopeError<E> {
    fn from(value: notify::Error<E>) -> Self {
        TelescopeError::NotifyError(value)
    }
}

impl<E> From<ChangeError<()>> for TelescopeError<E> {
    fn from(value: ChangeError<()>) -> Self {
        TelescopeError::ChangeError(value)
    }
}

impl<E> From<DeviceSelectionError> for TelescopeError<E> {
    fn from(value: DeviceSelectionError) -> Self {
        TelescopeError::DeviceSelectionError(value)
    }
}

impl<E> From<DeviceError> for TelescopeError<E> {
    fn from(value: DeviceError) -> Self {
        TelescopeError::DeviceError(value)
    }
}

#[derive(Debug)]
pub enum DeviceError {
    Notify(notify::Error<()>),
    TypeError(TypeError),
    ChangeError(ChangeError<()>),
    Missing,
    UnknownVarient,
    SendError(SendError<Command>),
    TimeoutError,
    BroadcastStreamRecvError,
}

impl From<BroadcastStreamRecvError> for DeviceError {
    fn from(_: BroadcastStreamRecvError) -> Self {
        DeviceError::BroadcastStreamRecvError
    }
}

impl From<twinkle_client::TimeoutError> for DeviceError {
    fn from(_: twinkle_client::TimeoutError) -> Self {
        DeviceError::TimeoutError
    }
}

impl From<notify::Error<()>> for DeviceError {
    fn from(value: notify::Error<()>) -> Self {
        DeviceError::Notify(value)
    }
}
impl From<TypeError> for DeviceError {
    fn from(value: TypeError) -> Self {
        DeviceError::TypeError(value)
    }
}

impl From<ChangeError<()>> for DeviceError {
    fn from(value: ChangeError<()>) -> Self {
        DeviceError::ChangeError(value)
    }
}

impl From<SendError<Command>> for DeviceError {
    fn from(value: SendError<Command>) -> Self {
        DeviceError::SendError(value)
    }
}

impl From<notify::Error<ChangeError<()>>> for DeviceError {
    fn from(value: notify::Error<ChangeError<()>>) -> Self {
        match value {
            notify::Error::Timeout => DeviceError::Notify(notify::Error::Timeout),
            notify::Error::Canceled => DeviceError::Notify(notify::Error::Canceled),
            notify::Error::EndOfStream => DeviceError::Notify(notify::Error::EndOfStream),
            notify::Error::Abort(_) => DeviceError::Notify(notify::Error::Abort(())),
        }
    }
}

pub struct Telescope {
    pub config: TelescopeConfig,
    pub client: Client,
    pub image_client: Client,

    pub agent: Agent<()>,
}

impl Telescope {
    pub fn new(config: TelescopeConfig) -> Telescope {
        let client = indi::client::Client::new(None);
        let image_client = indi::client::Client::new(None);
        let agent = Agent::default();
        Telescope {
            config,
            client,
            image_client,
            agent,
        }
    }

    pub async fn connect_from_settings(&mut self, settings: &Arc<Notify<Settings>>) {
        let settings = settings.read().await;
        self.connect(settings.indi_server_addr.clone()).await;
    }

    pub async fn connect(
        &mut self,
        addr: impl tokio::net::ToSocketAddrs + Clone + Display + Send + 'static,
    ) {
        self.agent.abort();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.client.reset(Some(tx)).await;
        let client_task = indi::client::start(
            self.client.get_devices().clone(),
            rx,
            TcpStream::connect(addr.clone())
                .await
                .expect(format!("Unable to connect to {}", addr).as_str()),
        );
        self.client
            .send(serialization::Command::GetProperties(GetProperties {
                version: INDI_PROTOCOL_VERSION.to_string(),
                device: None,
                name: None,
            }))
            .unwrap();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.image_client.reset(Some(tx)).await;
        let image_client_task = indi::client::start(
            self.image_client.get_devices().clone(),
            rx,
            TcpStream::connect(addr.clone())
                .await
                .expect(format!("Unable to connect to {}", addr).as_str()),
        );
        self.client
            .send(serialization::Command::GetProperties(GetProperties {
                version: INDI_PROTOCOL_VERSION.to_string(),
                device: Some(self.config.primary_camera.clone()),
                name: None,
            }))
            .unwrap();

        self.agent.spawn((), |_| async move {
            tokio::select! {
                _ = client_task => {},
                _ = image_client_task => {},
            }
        });
    }

    pub async fn get_primary_camera(&self) -> Result<Camera, TelescopeError<()>> {
        Ok(Camera::new(
            Self::get_device(&self.client, &self.config.primary_camera).await?,
            Self::get_device(&self.image_client, &self.config.primary_camera).await?,
        )
        .await?)
    }

    pub async fn get_filter_wheel(&self) -> Result<FilterWheel, TelescopeError<()>> {
        Ok(Self::get_device(&self.client, &self.config.filter_wheel)
            .await?
            .into())
    }

    pub async fn get_focuser(&self) -> Result<ActiveDevice, TelescopeError<()>> {
        Self::get_device(&self.client, &self.config.focuser).await
    }

    pub async fn get_flat_panel(&self) -> Result<FlatPanel, TelescopeError<()>> {
        let device = Self::get_device(&self.client, &self.config.flat_panel).await?;
        let flat_panel = flat_panel::FlatPanel::new(device).await?;
        Ok(flat_panel)
    }

    pub async fn join(&mut self) {
        let _ = self.agent.join().await;
    }

    async fn get_device(client: &Client, name: &str) -> Result<ActiveDevice, TelescopeError<()>> {
        Ok(client.get_device(name).await?)
    }
}

pub trait Connectable {
    fn connect(
        &self,
    ) -> impl std::future::Future<
        Output = Result<
            impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
            ChangeError<()>,
        >,
    > + Send;
}

impl Connectable for ActiveDevice {
    async fn connect(
        &self,
    ) -> Result<
        impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
        ChangeError<()>,
    > {
        self.change("CONNECTION", vec![("CONNECT", true)]).await
    }
}

#[derive(Debug)]
pub enum ParameterError<E> {
    Missing,
    TypeError(TypeError),
    ChangeError(ChangeError<E>),
}

impl<E> From<TypeError> for ParameterError<E> {
    fn from(value: TypeError) -> Self {
        ParameterError::TypeError(value)
    }
}

impl<E> From<ChangeError<E>> for ParameterError<E> {
    fn from(value: ChangeError<E>) -> Self {
        ParameterError::ChangeError(value)
    }
}
