use std::{fmt::Display, sync::Arc};

use camera::Camera;
use filter_wheel::FilterWheel;
use flat_panel::FlatPanel;
use indi::{
    client::{
        active_device::{ActiveDevice, SendError}, ChangeError, Client, ClientTask
    },
    serialization::Command,
    Parameter, TypeError,
};
use parameter_with_config::BlobParameter;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::Stream;
use twinkle_client::{
    notify::{self, ArcCounter},
    task::{Abortable, Joinable, Task, TaskStatusError},
};

pub mod camera;
pub mod filter_wheel;
pub mod flat_panel;
mod parameter_with_config;

#[derive(Debug)]
pub enum DeviceSelectionError {
    DeviceMismatch,
    UnkownDevice,
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
    pub client: ClientTask<Arc<tokio::sync::Mutex<Client>>>,
    pub image_client: ClientTask<Arc<tokio::sync::Mutex<Client>>>,
}

impl Telescope {
    pub async fn new(
        addr: impl tokio::net::ToSocketAddrs + Copy + Display + Send + 'static,
        config: TelescopeConfig,
    ) -> Telescope {
        let client = indi::client::new(
            TcpStream::connect(addr.clone())
                .await
                .expect(format!("Unable to connect to {}", addr).as_str()),
            None,
            None,
        );

        let image_client = indi::client::new(
        TcpStream::connect(addr.clone())
            .await
            .expect(format!("Unable to connect to {}", addr).as_str()),

            Some(&config.primary_camera.clone()),
            None,
        );

        Telescope {
            config,
            client,
            image_client,
        }
    }

    pub async fn get_primary_camera(&self) -> Result<Camera, TelescopeError<()>> {
        Ok(Camera::new(Self::get_device(&self.client, &self.config.primary_camera).await?).await?)
    }

    pub async fn get_primary_camera_ccd(&self) -> Result<BlobParameter, TelescopeError<()>> {
        let image_camera = Camera::new(Self::get_device(&self.image_client, &self.config.primary_camera).await?).await?;
        let image_param = image_camera.image().await?;
        image_param.enable_blob(indi::BlobEnable::Also).await?;
        Ok(image_param)
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
        tokio::select!(
            _ = self.client.join()=> self.image_client.abort(),
            _ = self.image_client.join() => self.client.abort()
        )
    }

    async fn get_device(
        client: &ClientTask<Arc<Mutex<Client>>>,
        name: &str,
    ) -> Result<ActiveDevice, TelescopeError<()>> {
        let running_status = client.running_status().await?;
        let client = running_status
            .with_state(|state| {
                let state = state.clone();
                async move { state.clone() }
            })
            .await
            .unwrap();
        drop(running_status);
        let lock = client.lock().await;
        Ok(lock.get_device(name).await?)
    }
}

pub struct OpticsConfig {
    pub focal_length: f64,
    pub aperture: f64,
}

pub struct TelescopeConfig {
    pub mount: String,
    pub primary_optics: OpticsConfig,
    pub primary_camera: String,
    pub focuser: String,
    pub filter_wheel: String,
    pub flat_panel: String,
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

pub trait Baseline {}
