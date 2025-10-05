use std::{collections::HashMap, marker::PhantomData, ops::Deref};

use crate::{
    client::{
        active_device::{ActiveDevice, SendError},
        active_parameter::ActiveParameter,
        ChangeError,
    },
    serialization::{Command, EnableBlob, OneNumber, OneSwitch, Sexagesimal, ToCommand},
    Blob, BlobEnable, FromParamValue, Number, Parameter, Switch, SwitchState, TryEq,
};
use itertools::Itertools;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::{Stream, StreamExt};
use twinkle_client::notify::{self, NotifyArc};

use super::DeviceError;

pub trait IntoValue {
    type Value: Clone;
    type SingleValue: Clone;

    fn into_value<T: Into<Self::SingleValue>>(&self, value: T) -> Self::Value;
}
pub trait FromParam {
    type Value;
    type Error;

    fn get_parameter_name(&self) -> &'static str;
    fn new(param: NotifyArc<Parameter>, config: Self) -> ParameterWithConfig<Self>
    where
        Self: Sized;

    fn from_parameter<T: Deref<Target = Parameter>>(
        &self,
        param: &T,
    ) -> Result<Self::Value, Self::Error>
    where
        Self: Sized;
}

pub (in crate::telescope) async fn get_parameter_value<T>(
    device: &ActiveDevice,
    parameter: &'static str,
    value: &'static str,
) -> Option<T>
where
    HashMap<String, T>: FromParamValue,
    T: Clone,
{
    device
        .parameter(parameter)
        .await
        .ok()?
        .get::<T>(value)
        .await
        .ok()
}

#[derive(Debug, Clone)]
pub struct SingleValueParamConfig<T: Clone> {
    pub parameter: &'static str,
    pub value: &'static str,
    _phantom: PhantomData<T>,
}

impl<T: Clone> SingleValueParamConfig<T>
where
    T: Clone,
    HashMap<String, T>: FromParamValue,
{
    pub fn new(parameter: &'static str, value: &'static str) -> Self {
        Self {
            parameter,
            value,
            _phantom: PhantomData::<T>,
        }
    }
}

pub struct ActiveParameterWithConfig<T> {
    pub parameter: ActiveParameter,
    config: T,
}

impl<T: Clone> Clone for ActiveParameterWithConfig<T> {
    fn clone(&self) -> Self {
        Self {
            parameter: self.parameter.clone(),
            config: self.config.clone(),
        }
    }
}

impl<T: FromParam> ActiveParameterWithConfig<T> {
    pub async fn new(device: &ActiveDevice, config: T) -> Result<Self, DeviceError> {
        let parameter = device.parameter(config.get_parameter_name()).await?;
        Ok(Self { parameter, config })
    }
}

pub struct ParameterWithConfig<T> {
    pub parameter: NotifyArc<Parameter>,
    pub config: T,
}

impl<T: Clone> Clone for ParameterWithConfig<T> {
    fn clone(&self) -> Self {
        Self {
            parameter: self.parameter.clone(),
            config: self.config.clone(),
        }
    }
}

impl<T> ParameterWithConfig<T>
where
    T: Clone,
    T: FromParam,
{
    pub fn get(&self) -> Result<<T as FromParam>::Value, <T as FromParam>::Error> {
        self.config.from_parameter(&self.parameter)
    }
}

impl<T> Deref for ParameterWithConfig<T> {
    type Target = Parameter;
    fn deref(&self) -> &Self::Target {
        self.parameter.deref()
    }
}

impl IntoValue for SingleValueParamConfig<Switch> {
    type SingleValue = SwitchState;
    type Value = Vec<OneSwitch>;

    fn into_value<T: Into<Self::SingleValue>>(&self, value: T) -> Self::Value {
        vec![OneSwitch {
            name: self.value.to_string(),
            value: value.into(),
        }]
    }
}

impl FromParam for SingleValueParamConfig<Switch> {
    type Value = Switch;
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
        parameter: &T,
    ) -> Result<Self::Value, Self::Error>
    where
        Self: Sized,
    {
        let values = parameter.deref().get_values::<HashMap<String, Switch>>()?;
        if let Some(value) = values.get(self.value) {
            return Ok(value.clone());
        }

        Err(DeviceError::Missing)
    }
}

impl IntoValue for SingleValueParamConfig<Number> {
    type Value = Vec<OneNumber>;
    type SingleValue = Sexagesimal;

    fn into_value<T: Into<Self::SingleValue>>(&self, value: T) -> Self::Value {
        vec![OneNumber {
            name: self.value.to_string(),
            value: value.into(),
        }]
    }
}

impl FromParam for SingleValueParamConfig<Number> {
    type Value = Number;
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
        let values = param.deref().get_values::<HashMap<String, Number>>()?;
        if let Some(value) = values.get(self.value) {
            return Ok(value.clone());
        }

        Err(DeviceError::Missing)
    }
}

impl FromParam for SingleValueParamConfig<Blob> {
    type Value = Blob;
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
        let values = param.deref().get_values::<HashMap<String, Blob>>()?;
        if let Some(value) = values.get(self.value) {
            return Ok(value.clone());
        }

        Err(DeviceError::Missing)
    }
}

impl<T> ActiveParameterWithConfig<T>
where
    T: Clone,
{
    /// Create a stream of values for the parameter.  This behaives like the change method, but yields the
    /// current value immediately.
    pub async fn subscribe(
        &self,
    ) -> impl Stream<Item = Result<ParameterWithConfig<T>, BroadcastStreamRecvError>> + use<'_, T>
    {
        self.parameter.subscribe().await.map(|parameter| {
            Ok(ParameterWithConfig {
                parameter: parameter?,
                config: self.config.clone(),
            })
        })
    }

    /// Create a stream of values for the parameter.  Each time the parameter changes this stream
    /// will yield the new value.
    pub fn changes(
        &self,
    ) -> impl Stream<Item = Result<ParameterWithConfig<T>, BroadcastStreamRecvError>> + use<T> {
        let config = self.config.clone();
        self.parameter.changes().map(move |parameter| {
            Ok(ParameterWithConfig {
                parameter: parameter?,
                config: config.clone(),
            })
        })
    }
}

impl<T> ActiveParameterWithConfig<T>
where
    T: Clone,
    T: FromParam,
{
    /// Get the current value of the parameter.
    pub async fn get(&self) -> Result<<T as FromParam>::Value, <T as FromParam>::Error> {
        let parameter = self.parameter.read().await;
        self.config.from_parameter(&parameter)
    }
}

impl<T> ActiveParameterWithConfig<T>
where
    T: Clone,
    T: IntoValue + FromParam,
{
    /// Request a change to the current value, and await until the parameter has the desired value.
    pub async fn change<V>(
        &self,
        values: V,
    ) -> Result<
        impl Stream<Item = Result<ParameterWithConfig<T>, BroadcastStreamRecvError>> + use<'_, T, V>,
        ChangeError<()>,
    >
    where
        V: Into<<T as IntoValue>::SingleValue>,
        <T as IntoValue>::Value: TryEq<Parameter>,
        <T as IntoValue>::Value: ToCommand,
    {
        Ok(self
            .parameter
            .change(self.config.into_value(values))
            .await?
            .map(|parameter| Ok(T::new(parameter?, self.config.clone()))))
    }

    /// Request a change to the current value.
    pub fn set<V>(&self, values: V) -> Result<(), SendError<Command>>
    where
        V: Into<<T as IntoValue>::SingleValue>,
        <T as IntoValue>::Value: TryEq<Parameter>,
        <T as IntoValue>::Value: ToCommand,
    {
        self.parameter.set(self.config.into_value(values))
    }
}

#[derive(derive_more::Deref, derive_more::From)]
pub struct NumberParameter(ActiveParameterWithConfig<SingleValueParamConfig<Number>>);

#[derive(derive_more::Deref, derive_more::From)]
pub struct SwitchParameter(ActiveParameterWithConfig<SingleValueParamConfig<Switch>>);

#[derive(derive_more::Deref, derive_more::From)]
pub struct BlobParameter(ActiveParameterWithConfig<SingleValueParamConfig<Blob>>);

impl BlobParameter {
    pub async fn enable_blob(&self, enabled: BlobEnable) -> Result<(), notify::Error<()>> {
        if let Err(_) = self.parameter.set(Command::EnableBlob(EnableBlob {
            device: self.parameter.get_device().get_name().clone(),
            name: Some(self.config.value.to_string()),
            enabled,
        })) {
            return Err(notify::Error::Canceled);
        };
        Ok(())
    }
}

#[derive(Clone)]
pub struct OneOfMany<T> {
    parameter: &'static str,
    mapping: HashMap<&'static str, T>,
}

impl<T: Clone + PartialEq> OneOfMany<T> {
    pub fn new(parameter: &'static str, mapping: HashMap<&'static str, T>) -> Self {
        Self { parameter, mapping }
    }
    fn from_enum(&self, state: T) -> Option<&'static str> {
        self.mapping
            .iter()
            .find_or_first(|(_, value)| **value == state)
            .map(|(name, _)| *name)
    }

    fn to_enum<'a>(&self, str: &'a str) -> Option<T> {
        self.mapping.get(str).cloned()
    }
}

impl<T: Clone + PartialEq> IntoValue for OneOfMany<T> {
    type Value = Vec<OneSwitch>;
    type SingleValue = T;

    fn into_value<V: Into<Self::SingleValue>>(&self, value: V) -> Self::Value {
        let enabled = value.into();
        let name = self.from_enum(enabled).unwrap();
        return vec![OneSwitch {
            name: name.to_string(),
            value: SwitchState::On,
        }];
    }
}

impl<T: Clone + PartialEq> FromParam for OneOfMany<T> {
    type Value = T;
    type Error = DeviceError;

    fn get_parameter_name(&self) -> &'static str {
        self.parameter
    }

    fn new(param: NotifyArc<Parameter>, config: Self) -> ParameterWithConfig<Self>
    where
        Self: Sized,
    {
        ParameterWithConfig {
            parameter: param,
            config,
        }
    }

    fn from_parameter<V: Deref<Target = Parameter>>(
        &self,
        param: &V,
    ) -> Result<Self::Value, Self::Error>
    where
        Self: Sized,
    {
        for (name, value) in param.get_values::<HashMap<String, Switch>>()? {
            if let SwitchState::On = value.value {
                return match self.to_enum(name.as_str()) {
                    Some(value) => Ok(value),
                    None => Err(DeviceError::UnknownVarient),
                };
            }
        }
        return Err(DeviceError::Missing);
    }
}
