use crate::telescope::parameter_with_config::{ActiveParameterWithConfig, OneOfMany};

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum TransferFormat {
    Native,
    Xisf,
    Fits,
}

#[derive(derive_more::Deref, derive_more::Into, derive_more::From)]
pub struct TransferFormatParameter(ActiveParameterWithConfig<OneOfMany<TransferFormat>>);
