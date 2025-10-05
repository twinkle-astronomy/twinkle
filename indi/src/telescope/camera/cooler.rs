use crate::telescope::parameter_with_config::{ActiveParameterWithConfig, OneOfMany};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cooler {
    On,
    Off,
}

impl From<bool> for Cooler {
    fn from(value: bool) -> Self {
        match value {
            true => Cooler::On,
            false => Cooler::Off,
        }
    }
}

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct CoolerParameter(ActiveParameterWithConfig<OneOfMany<Cooler>>);
