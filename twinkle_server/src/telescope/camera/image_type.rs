use crate::telescope::parameter_with_config::{ActiveParameterWithConfig, OneOfMany};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageType {
    Flat,
    Dark,
    Bias,
    Light,
}

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct ImageTypeParameter(ActiveParameterWithConfig<OneOfMany<ImageType>>);
