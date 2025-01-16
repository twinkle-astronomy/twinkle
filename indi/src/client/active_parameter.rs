use super::{active_device, ChangeError};
use crate::{Command, Parameter, ToCommand, TryEq};
use std::{ops::Deref, sync::Arc};
use twinkle_client::notify::Notify;

#[derive(Clone)]
pub struct ActiveParameter {
    device: active_device::ActiveDevice,
    parameter: Arc<Notify<Parameter>>,
}

impl ActiveParameter {
    pub fn new(
        device: active_device::ActiveDevice,
        parameter: Arc<Notify<Parameter>>,
    ) -> ActiveParameter {
        ActiveParameter { device, parameter }
    }

    pub async fn change<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static>(
        &self,
        values: P,
    ) -> Result<Arc<Parameter>, ChangeError<Command>> {
        self.device
            .change(
                self.parameter.lock().await.get_name().clone().as_str(),
                values,
            )
            .await
    }
}

impl Deref for ActiveParameter {
    type Target = Arc<Notify<Parameter>>;

    fn deref(&self) -> &Self::Target {
        &self.parameter
    }
}
