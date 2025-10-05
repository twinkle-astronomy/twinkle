use std::{ops::Deref, sync::Arc, time::Duration};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, Stream};

use twinkle_client::notify::{self, wait_fn, ArcCounter, Notify};

use crate::{device::Device, serialization, Command, EnableBlob, Parameter, ToCommand, TryEq};

use super::{active_parameter::ActiveParameter, ChangeError};

#[derive(Debug)]
pub enum SendError<T> {
    Disconnected,
    SendError(tokio::sync::mpsc::error::SendError<T>),
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for SendError<T> {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        SendError::SendError(value)
    }
}

/// Object representing a device connected to an INDI server.
#[derive(Clone)]
pub struct ActiveDevice {
    name: String,
    device: Arc<Notify<Device>>,
    command_sender: Option<tokio::sync::mpsc::UnboundedSender<serialization::Command>>,
}

impl ActiveDevice {
    pub fn new(
        name: String,
        device: Arc<Notify<Device>>,
        command_sender: Option<tokio::sync::mpsc::UnboundedSender<serialization::Command>>,
    ) -> ActiveDevice {
        ActiveDevice {
            name,
            device,
            command_sender,
        }
    }

    pub fn send(&self, c: Command) -> Result<(), SendError<Command>> {
        if let Some(command_sender) = &self.command_sender {
            command_sender.send(c)?;
        }
        Ok(())
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }
}

impl Deref for ActiveDevice {
    type Target = Arc<Notify<Device>>;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl ActiveDevice {
    /// Returns the requested parameter, waiting up to 1 second for it to be defined
    ///  by the connected INDI server.  
    pub async fn get_parameter(
        &self,
        param_name: &str,
    ) -> Result<Arc<Notify<Parameter>>, notify::Error<()>> {
        let mut subs = self.device.subscribe().await;
        wait_fn(&mut subs, Duration::from_secs(1), |device| {
            Ok(match device.get_parameters().get(param_name) {
                Some(param) => notify::Status::Complete(param.clone()),
                None => notify::Status::Pending,
            })
        })
        .await
    }

    pub async fn parameter(&self, param_name: &str) -> Result<ActiveParameter, notify::Error<()>> {
        let mut subs = self.device.subscribe().await;
        wait_fn(&mut subs, Duration::from_secs(1), |device| {
            Ok(match device.get_parameters().get(param_name) {
                Some(param) => notify::Status::Complete(ActiveParameter::new(
                    param_name.to_string(),
                    self.clone(),
                    param.clone(),
                )),
                None => notify::Status::Pending,
            })
        })
        .await
    }

    /// Ensures that the parameter named `param_name` has the given value with the INDI server.
    /// If the INDI server's value does not match the `values` given, it will send the
    /// INDI server commands necessary to change values, and wait for the server
    /// to confirm the desired values.  This method will wait for the parameter's
    /// `timeout` (or 60 seconds if not defined by the server) for the parameter value to match
    ///  the desired value before timing out.
    /// # Arguments
    /// * `param_name` - The name of the parameter you wish to change.  If the parameter does not exist,
    ///                  This method will wait up to 1 second for it to exist before timing out.
    /// * `values` - The target values of the named parameter.  This argument must be of a type that
    // /              can be compared to the named parameter, and converted into an INDI command if nessesary.
    // /              See [crate::TryEq] and [crate::ToCommand] for type conversions.  If the given values do not
    // /              match the parameter types nothing be communicated to the server and aa [ChangeError::TypeMismatch]
    // /              will be returned.
    // / # Example
    // / ```no_run
    // / use indi::*;
    // / use indi::client::active_device::ActiveDevice;
    // / async fn change_usage_example(filter_wheel: ActiveDevice) {
    // /     filter_wheel.change(
    // /         "FILTER_SLOT",
    // /         vec![("FILTER_SLOT_VALUE", 5.0)],
    // /     ).await.expect("Changing filter");
    // / }
    /// ```
    pub async fn change<'a, P: Clone + TryEq<Parameter> + ToCommand>(
        &'a self,
        param_name: &'a str,
        values: P,
    ) -> Result<
        impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
        ChangeError<()>,
    > {
        let param = self.parameter(param_name).await?;
        param.change(values).await
    }

    /// Sends an `EnableBlob` command to the connected INDI server for the named parameter.  This must be called
    ///  on a Blob parameter with a value of either [crate::BlobEnable::Only] or [crate::BlobEnable::Also] for
    ///  the server to send image data.
    /// # Arguments
    /// * `param_name` - The optional name of the blob parameter to configure.  If `Some(param_name)` is provided
    ///                  and the parameter does not exist, this method will wait up to 1 second for it to exist
    ///                  before timing out.
    /// * `enabled` - The [crate::BlobEnable] value you wish to send to the server.
    // / # Example
    // / ```no_run
    // / use indi::client::active_device::ActiveDevice;
    // / use indi::BlobEnable;
    // / async fn enable_blob_usage_example(ccd: ActiveDevice) {
    // /     // Instruct server to send blob data along with regular parameter updates
    // /     camera.enable_blob(
    // /         Some("CCD1"), BlobEnable::Also,
    // /     ).await.expect("Enabling blobs");
    // / }
    // / ```
    pub async fn enable_blob(
        &self,
        name: Option<&str>,
        enabled: crate::BlobEnable,
    ) -> Result<(), notify::Error<()>> {
        // Wait for device and paramater to exist
        if let Some(name) = name {
            let _ = self.get_parameter(name).await?;
        }
        let device_name = self.device.read().await.get_name().clone();
        if let Err(_) = self.send(Command::EnableBlob(EnableBlob {
            device: device_name,
            name: name.map(|x| String::from(x)),
            enabled,
        })) {
            return Err(notify::Error::Canceled);
        };
        Ok(())
    }
}
