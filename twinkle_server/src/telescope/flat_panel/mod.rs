
use indi::{
    client::active_device::ActiveDevice, Number, Text
};

use super::{
    parameter_with_config::{
        get_parameter_value, ActiveParameterWithConfig, NumberParameter, OneOfMany, SingleValueParamConfig
    },
    DeviceError, DeviceSelectionError,
};

#[derive(Clone, Debug, PartialEq)]
pub enum LightState {
    On,
    Off,
}

impl From<bool> for LightState {
    fn from(value: bool) -> Self {
        match value {
            true => LightState::On,
            false => LightState::Off,
        }
    }
}

pub struct FlatPanel {
    device: ActiveDevice,
    config: FlatPanelConfig,
}

struct FlatPanelConfig {
    brightness: SingleValueParamConfig<Number>,
    light: OneOfMany<LightState>,
}


#[derive(derive_more::Deref)]
pub struct Light(ActiveParameterWithConfig<OneOfMany<LightState>>);


impl FlatPanel {
    async fn get_driver_name(device: &ActiveDevice) -> Result<Text, super::DeviceSelectionError> {
        if let Some(driver_name) = get_parameter_value(device, "DRIVER_INFO", "DRIVER_NAME").await {
            return Ok(driver_name);
        }
        Err(DeviceSelectionError::DeviceMismatch)
    }

    fn get_config(driver_name: &Text) -> Result<FlatPanelConfig, super::DeviceSelectionError> {
        match driver_name.value.as_str() {
            "Light Panel Simulator" => Ok(FlatPanelConfig {
                brightness: SingleValueParamConfig::new(
                    "FLAT_LIGHT_INTENSITY",
                    "FLAT_LIGHT_INTENSITY_VALUE",
                ),
                light: OneOfMany::new(
                    "FLAT_LIGHT_CONTROL",
                    [("FLAT_LIGHT_ON", LightState::On), ("FLAT_LIGHT_OFF", LightState::Off)].into_iter().collect()
                ),
            }),
            "Deep Sky Dad FP" => Ok(FlatPanelConfig {
                brightness: SingleValueParamConfig::new(
                    "FLAT_LIGHT_INTENSITY",
                    "FLAT_LIGHT_INTENSITY_VALUE",
                ),
                light: OneOfMany::new(
                    "FLAT_LIGHT_CONTROL",
                    [("FLAT_LIGHT_ON", LightState::On), ("FLAT_LIGHT_OFF", LightState::Off)].into_iter().collect()
                ),
            }),
            _ => Err(DeviceSelectionError::DeviceMismatch),
        }
    }

    pub async fn new(device: ActiveDevice) -> Result<Self, super::DeviceSelectionError> {
        let driver_name = Self::get_driver_name(&device).await?;
        let config = FlatPanel::get_config(&driver_name)?;

        Ok(FlatPanel { device, config })
    }
    pub async fn brightness(&self) -> Result<NumberParameter, DeviceError> {
        Ok(ActiveParameterWithConfig::new(
            &self.device,
            self.config.brightness.clone(),
        )
        .await?
        .into())
    }

    pub async fn light(&self) -> Result<Light, DeviceError> {
        let apwc = ActiveParameterWithConfig::new(
            &self.device, self.config.light.clone()).await?;
        Ok(Light(apwc))
    }
}


pub struct Config {
    pub is_on: bool,
}

impl Config {
    pub async fn set(&self, flat_panel: &FlatPanel) -> Result<(), DeviceError> {
        let light = flat_panel.light().await?;
        let _ = light.change(self.is_on).await?;
        Ok(())
    }
}

