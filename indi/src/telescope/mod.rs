use std::ops::Deref;

use crate::client::{AsyncClientConnection, Connectable};
use crate::{
    client::{
        active_device::{ActiveDevice, SendError},
        ChangeError, Client,
    },
    serialization::{self, Command, GetProperties},
    telescope::settings::{Settings, TelescopeConfig},
    TypeError, INDI_PROTOCOL_VERSION,
};
use camera::Camera;
use filter_wheel::FilterWheel;
use flat_panel::FlatPanel;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use twinkle_client::{
    agent::Agent,
    notify,
    task::{Abortable, Joinable, TaskStatusError},
    MaybeSend,
};
pub mod camera;
pub mod filter;
pub mod filter_wheel;
pub mod flat_panel;
pub mod settings;

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

/// Type representing an indi telescope.
pub struct Telescope {
    config: TelescopeConfig,
    client: Client,
    image_client: Client,

    agent: Agent<()>,
}

impl Telescope {
    /// Create an instance of a telescope.  
    pub fn new(config: TelescopeConfig) -> Telescope {
        let client = crate::client::Client::new(None);
        let image_client = crate::client::Client::new(None);
        let agent = Agent::default();
        Telescope {
            config,
            client,
            image_client,
            agent,
        }
    }

    /// Connect telescope instance to indi server.
    pub async fn connect_from_settings<
        T: Connectable + AsyncClientConnection + 'static + MaybeSend,
    >(
        &mut self,
        settings: impl Deref<Target = Settings>,
    ) {
        let settings = settings.deref();
        self.connect::<T>(settings.indi_server_addr.clone()).await;
    }

    /// Connect telescope instance to indi server at given address.
    pub async fn connect<T: Connectable + AsyncClientConnection + 'static + MaybeSend>(
        &mut self,
        addr: String,
    ) {
        self.agent.abort();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.client.reset(Some(tx)).await;
        let client_task = crate::client::start(
            self.client.get_devices().clone(),
            rx,
            T::connect(addr.clone())
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
        let image_client_task = crate::client::start(
            self.image_client.get_devices().clone(),
            rx,
            T::connect(addr.clone())
                .await
                .expect(format!("Unable to connect to {}", addr).as_str()),
        );
        self.image_client
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

    /// Get primary camera for the telescope.
    pub async fn get_primary_camera(&self) -> Result<Camera, TelescopeError<()>> {
        let device = Self::get_device(&self.client, &self.config.primary_camera).await?;
        let _ = device.connect().await.unwrap();
        let ccd_device = Self::get_device(&self.image_client, &self.config.primary_camera).await?;
        Ok(Camera::new(device, ccd_device).await?)
    }

    /// Get filter wheel for the telescope.
    pub async fn get_filter_wheel(&self) -> Result<FilterWheel, TelescopeError<()>> {
        let device = Self::get_device(&self.client, &self.config.filter_wheel).await?;
        let _ = device.connect().await.unwrap();
        Ok(device.into())
    }

    /// Get focuser for the telescope.
    pub async fn get_focuser(&self) -> Result<ActiveDevice, TelescopeError<()>> {
        let device = Self::get_device(&self.client, &self.config.focuser).await?;
        let _ = device.connect().await.unwrap();
        Ok(device)
    }

    /// Get the flat panel for the telescope.
    pub async fn get_flat_panel(&self) -> Result<FlatPanel, TelescopeError<()>> {
        let device = Self::get_device(&self.client, &self.config.flat_panel).await?;
        let _ = device.connect().await.unwrap();
        let flat_panel = flat_panel::FlatPanel::new(device).await?;
        Ok(flat_panel)
    }

    /// Wait for disconnect.
    pub async fn join(&mut self) {
        let _ = self.agent.join().await;
    }

    async fn get_device(client: &Client, name: &str) -> Result<ActiveDevice, TelescopeError<()>> {
        Ok(client.get_device(name).await?)
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
