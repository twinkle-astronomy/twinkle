use std::{collections::HashMap, ops::Deref};

use crate::{
    client::{active_device::ActiveDevice, ChangeError},
    serialization::{OneText, Sexagesimal},
    telescope::filter::Filter,
    Number, Parameter, Text,
};
use derive_more::Debug;
use futures::Stream;
use itertools::Itertools;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use twinkle_client::notify::{ArcCounter, NotifyArc};

use super::{
    parameter_with_config::{
        ActiveParameterWithConfig, FromParam, IntoValue, NumberParameter, ParameterWithConfig,
        SingleValueParamConfig,
    },
    DeviceError,
};

#[derive(Debug)]
pub enum FlatError {
    DeviceError(DeviceError),
    FilterNotFound(String),
}

impl From<DeviceError> for FlatError {
    fn from(value: DeviceError) -> Self {
        FlatError::DeviceError(value)
    }
}

/// Type representing the filter wheel in a telescope
pub struct FilterWheel {
    device: ActiveDevice,
    config: FilterWheelConfig,
}

struct FilterWheelConfig {
    filter_slot: SingleValueParamConfig<Number>,
    filter_list: FilterListParamConfig,
}

impl FilterWheel {
    /// Get parameter representing the filters currently configured.
    pub async fn filters(&self) -> Result<FilterListParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.filter_list.clone())
                .await?
                .into(),
        )
    }

    /// Get a filter by name.
    pub async fn get_filter(&self, name: &str) -> Result<Filter, FlatError> {
        let filters = self.filters().await?.get().await?;
        filters
            .into_iter()
            .filter(|x| x.name == name)
            .next()
            .ok_or(FlatError::FilterNotFound(name.to_string()))
    }

    async fn filter_slot(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.filter_slot.clone())
                .await?
                .into(),
        )
    }

    /// Connect to the filter wheel.
    pub async fn connect(
        &self,
    ) -> Result<
        impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
        ChangeError<()>,
    > {
        self.device
            .change("CONNECTION", vec![("CONNECT", true)])
            .await
    }
}

impl From<ActiveDevice> for FilterWheel {
    fn from(value: ActiveDevice) -> Self {
        FilterWheel {
            device: value,
            config: FilterWheelConfig {
                filter_slot: SingleValueParamConfig::new("FILTER_SLOT", "FILTER_SLOT_VALUE"),
                filter_list: FilterListParamConfig {
                    parameter: "FILTER_NAME",
                },
            },
        }
    }
}

pub struct Config {
    pub filter: Filter,
}

impl Config {
    pub async fn set(&self, filter_wheel: &FilterWheel) -> Result<(), DeviceError> {
        let filter_slot_param = filter_wheel.filter_slot().await?;

        let _ = filter_slot_param.change(self.filter.clone()).await?;

        Ok(())
    }
}
impl Into<Sexagesimal> for Filter {
    fn into(self) -> Sexagesimal {
        self.position.into()
    }
}

pub type FilterListParameter = ActiveParameterWithConfig<FilterListParamConfig>;

#[derive(Debug, Clone)]
pub struct FilterListParamConfig {
    pub parameter: &'static str,
}

impl IntoValue for FilterListParamConfig {
    type Value = Vec<OneText>;
    type SingleValue = Vec<Filter>;

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
    type Value = Vec<Filter>;
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
        let values: Vec<Filter> = values
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
                }
                .into()
            })
            .sorted_by(|lhs: &Filter, rhs: &Filter| lhs.position.cmp(&rhs.position))
            .collect();
        Ok(values)
    }
}
