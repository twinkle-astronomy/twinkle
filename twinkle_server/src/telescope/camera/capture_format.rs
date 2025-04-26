use crate::telescope::parameter_with_config::{ActiveParameterWithConfig, OneOfMany};


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureFormat {
    Raw8,
    Raw16,
}

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct CaptureFormatParameter(ActiveParameterWithConfig<OneOfMany<CaptureFormat>>);
