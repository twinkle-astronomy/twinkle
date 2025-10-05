use super::active_device::{ActiveDevice, SendError};
use super::{active_device, ChangeError};
use crate::serialization::Command;
use crate::{FromParamValue, Parameter, PropertyState, ToCommand, TryEq, TypeError};
use std::collections::HashMap;
use std::{ops::Deref, sync::Arc, time::Duration};
use tokio_stream::Stream;
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt};
use twinkle_client::notify::{self, wait_fn, ArcCounter, Notify};

#[derive(Clone)]
pub struct ActiveParameter {
    name: String,
    device: active_device::ActiveDevice,
    pub parameter: Arc<Notify<Parameter>>,
}

pub trait IntoValue {
    type Value: Clone + TryEq<Parameter> + ToCommand;

    fn into_value(self) -> Self::Value;
}

pub trait FromParameter: Sized {
    type Error;
    fn from_parameter<T: Deref<Target = Parameter>>(param: &T) -> Result<Self, Self::Error>;
}

impl ActiveParameter {
    pub fn new(
        name: String,
        device: active_device::ActiveDevice,
        parameter: Arc<Notify<Parameter>>,
    ) -> ActiveParameter {
        ActiveParameter {
            device,
            parameter,
            name,
        }
    }

    pub async fn get<T>(&self, value_name: &str) -> Result<T, TypeError>
    where
        HashMap<String, T>: FromParamValue,
        T: Clone,
    {
        let lock = self.read().await;
        let values = lock.get_values::<HashMap<String, T>>()?;
        match values.get(value_name) {
            Some(value) => Ok(value.clone()),
            None => Err(TypeError::TypeMismatch),
        }
    }

    pub fn set<'a, T>(&'a self, values: T) -> Result<(), SendError<Command>>
    where
        T: ToCommand,
    {
        self.device
            .send(values.to_command(self.device.get_name().clone(), self.name.clone()))
    }

    pub async fn change<'a, P: Clone + TryEq<Parameter> + ToCommand>(
        &'a self,
        values: P,
    ) -> Result<
        impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
        ChangeError<()>,
    > {
        let device_name = self.device.get_name().clone();

        let (mut subscription, timeout) = {
            let mut subscription = self.subscribe().await;
            let timeout = {
                let param = subscription.next().await.unwrap().unwrap();
                if !values.try_eq(&param)? {
                    let c = values
                        .clone()
                        .to_command(device_name, param.get_name().clone());
                    self.device.send(c)?;
                } else {
                    tracing::debug!(
                        "Skipping due to equality: {}.{}",
                        &device_name,
                        &param.get_name()
                    );
                    return Ok(subscription);
                }

                param.get_timeout().unwrap_or(60)
            }
            .max(1);
            (subscription, timeout)
        };

        wait_fn::<_, ChangeError<()>, _, _, _>(
            &mut subscription,
            Duration::from_secs(timeout.into()),
            move |next| {
                if *next.get_state() == PropertyState::Alert {
                    return Err(ChangeError::PropertyError);
                }
                if *next.get_state() == PropertyState::Busy {
                    return Ok(notify::Status::Pending);
                }
                if values.try_eq(&next)? {
                    Ok(notify::Status::Complete(next.clone()))
                } else {
                    Ok(notify::Status::Pending)
                }
            },
        )
        .await?;

        Ok(subscription)
    }

    pub fn get_device(&self) -> &ActiveDevice {
        &self.device
    }
}

impl Deref for ActiveParameter {
    type Target = Arc<Notify<Parameter>>;

    fn deref(&self) -> &Self::Target {
        &self.parameter
    }
}
