use std::{collections::HashMap, ops::Deref};

use indi::{
    client::active_device::ActiveDevice,
    serialization::{OneText, Sexagesimal},
    Number, Parameter, Text,
};
use itertools::Itertools;
use twinkle_api::Filter;
use twinkle_client::notify::NotifyArc;

use super::{
    parameter_with_config::{
        ActiveParameterWithConfig, FromParam, IntoValue, NumberParameter,
        ParameterWithConfig, SingleValueParamConfig,
    },
    DeviceError,
};

pub struct FilterWheel {
    device: ActiveDevice,
    config: FilterWheelConfig,
}

struct FilterWheelConfig {
    filter_slot: SingleValueParamConfig<Number>,
    filter_list: FilterListParamConfig,
}

impl FilterWheel {
    pub async fn filters(&self) -> Result<FilterListParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.filter_list.clone())
                .await?
                .into(),
        )
    }

    pub async fn filter_slot(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.filter_slot.clone())
                .await?
                .into(),
        )
    }
}

impl super::Connectable for FilterWheel {
    async fn connect(
        &self,
    ) -> Result<
        impl futures::Stream<Item = Result<twinkle_client::notify::ArcCounter<indi::Parameter>, tokio_stream::wrappers::errors::BroadcastStreamRecvError>>,
        indi::client::ChangeError<()>,
    > {
        self.device.change("CONNECTION", vec![("CONNECT", true)]).await
    }
}

impl From<ActiveDevice> for FilterWheel {
    fn from(value: ActiveDevice) -> Self {
        FilterWheel {
            device: value,
            config: FilterWheelConfig {
                filter_slot: SingleValueParamConfig::new("FILTER_SLOT", "FILTER_SLOT_VALUE"),
                filter_list: FilterListParamConfig { parameter: "FILTER_NAME" }
            },
        }
    }
}



pub struct Config {
    pub filter: TelescopeFilter,
}

impl Config {
    pub async fn set(&self, filter_wheel: &FilterWheel) -> Result<(), DeviceError> {
        let filter_slot_param = filter_wheel.filter_slot().await?;

        let _ = filter_slot_param.change(self.filter.clone()).await?;

        Ok(())
    }
}
impl Into<Sexagesimal> for TelescopeFilter {
    fn into(self) -> Sexagesimal {
        self.position.into()
    }
}

#[derive(Clone, Debug, derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct TelescopeFilter(Filter);

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct FilterListParameter(ActiveParameterWithConfig<FilterListParamConfig>);

#[derive(Debug, Clone)]
pub struct FilterListParamConfig {
    pub parameter: &'static str,
}

impl IntoValue for FilterListParamConfig {
    type Value = Vec<OneText>;
    type SingleValue = Vec<TelescopeFilter>;

    fn into_value<T: Into<Self::SingleValue>>(&self, value: T) -> Self::Value {
        let values: Self::SingleValue = value.into();

        values
            .into_iter()
            .map(|x| OneText {
                name: format!("FILTER_SLOT_NAME_{}", x.position),
                value: x.name.clone(),
            })
            .collect()
    }
}

impl FromParam for FilterListParamConfig {
    type Value = Vec<TelescopeFilter>;
    type Error = DeviceError;

    fn get_parameter_name(&self) -> &'static str {
        self.parameter
    }

    fn new(parameter: NotifyArc<Parameter>, config: Self) -> ParameterWithConfig<Self>
    where
        Self: Sized,
    {
        ParameterWithConfig { parameter, config }
    }

    fn from_parameter<T: Deref<Target = Parameter>>(
        &self,
        param: &T,
    ) -> Result<Self::Value, Self::Error>
    where
        Self: Sized,
    {
        let values = param.deref().get_values::<HashMap<String, Text>>()?;
        let values: Vec<TelescopeFilter> = values
            .iter()
            .map(|(slot, name)| {
                let slot = slot
                    .split("_")
                    .last()
                    .map(|x| x.parse::<usize>().unwrap())
                    .unwrap();
                Filter {
                    name: name.value.clone(),
                    position: slot,
                }.into()
            })
            .sorted_by(|lhs: &TelescopeFilter, rhs: &TelescopeFilter| lhs.position.cmp(&rhs.position))
            .collect();
        Ok(values)
    }
}
