use std::{collections::HashMap, ops::Deref};

use indi::{serialization::OneNumber, Number, Parameter};
use twinkle_client::notify::NotifyArc;

use crate::telescope::parameter_with_config::{
    ActiveParameterWithConfig, FromParam, IntoValue, ParameterWithConfig,
};

use super::DeviceError;

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct BinningParameter(ActiveParameterWithConfig<BinningConfig>);

#[derive(Clone, Copy)]
pub struct Binning {
    pub hor: u8,
    pub ver: u8,
}

#[derive(Clone)]
pub struct BinningConfig {
    parameter: &'static str,
    hor: &'static str,
    ver: &'static str,
}

impl BinningConfig {
    pub fn new(parameter: &'static str, hor: &'static str, ver: &'static str) -> Self {
        BinningConfig {
            parameter,
            hor,
            ver,
        }
    }
}

impl IntoValue for BinningConfig {
    type Value = Vec<OneNumber>;
    type SingleValue = Binning;

    fn into_value<T: Into<Self::SingleValue>>(&self, value: T) -> Self::Value {
        let single_value = value.into();
        vec![
            OneNumber {
                name: self.ver.to_string(),
                value: single_value.ver.into(),
            },
            OneNumber {
                name: self.hor.to_string(),
                value: single_value.hor.into(),
            },
        ]
    }
}

impl FromParam for BinningConfig {
    type Value = Binning;
    type Error = DeviceError;

    fn new(param: NotifyArc<Parameter>, config: Self) -> ParameterWithConfig<Self> {
        ParameterWithConfig {
            parameter: param,
            config,
        }
    }
    fn from_parameter<T: Deref<Target = Parameter>>(
        &self,
        param: &T,
    ) -> Result<Self::Value, Self::Error>
    where
        Self: Sized,
    {
        let values = param.deref().get_values::<HashMap<String, Number>>()?;
        let hor: f64 = values
            .get(self.hor)
            .ok_or(DeviceError::Missing)?
            .value
            .into();
        let ver: f64 = values
            .get(self.ver)
            .ok_or(DeviceError::Missing)?
            .value
            .into();
        Ok(Binning {
            hor: hor as u8,
            ver: ver as u8,
        })
    }

    fn get_parameter_name(&self) -> &'static str {
        self.parameter
    }
}
