use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use binning::{BinningConfig, BinningParameter};
use capture_format::CaptureFormatParameter;
use cooler::CoolerParameter;
use image_type::ImageTypeParameter;
use indi::{
    client::{active_device::ActiveDevice, wait_fn, ChangeError},
    Blob, Number, PropertyState, Switch, Text,
};
use tokio_stream::StreamExt;
use transfer_format::TransferFormatParameter;
use twinkle_client::notify;

use super::{
    parameter_with_config::{
        get_parameter_value, ActiveParameterWithConfig, BlobParameter, NumberParameter, OneOfMany,
        SingleValueParamConfig, SwitchParameter,
    },
    DeviceError, DeviceSelectionError,
};

mod transfer_format;
pub use transfer_format::TransferFormat;

mod capture_format;
pub use capture_format::CaptureFormat;

mod cooler;
pub use cooler::Cooler;

mod binning;
pub use binning::Binning;

mod image_type;
pub use image_type::ImageType;

pub struct Camera {
    device: ActiveDevice,
    config: CameraConfig,
}

struct CameraConfig {
    capture_format: OneOfMany<capture_format::CaptureFormat>,
    transfer_format: OneOfMany<transfer_format::TransferFormat>,
    cooler: OneOfMany<cooler::Cooler>,
    tempurature: SingleValueParamConfig<Number>,
    gain: SingleValueParamConfig<Number>,
    offset: SingleValueParamConfig<Number>,
    image_type: OneOfMany<image_type::ImageType>,
    binning: BinningConfig,
    exposure: SingleValueParamConfig<Number>,
    image: SingleValueParamConfig<Blob>,
    abort: SingleValueParamConfig<Switch>,
}

pub struct Config {
    pub bit_depth: CaptureFormat,
    pub transfer_format: TransferFormat,
    pub image_type: ImageType,
    pub binning: u8,
    pub gain: f64,
    pub offset: f64,
    pub tempurature: Option<f64>,
}

impl Config {
    pub async fn set(&self, camera: &Camera) -> Result<(), DeviceError> {
        let _ = camera
            .capture_format()
            .await?
            .change(self.bit_depth)
            .await?;
        let _ = camera
            .transfer_format()
            .await?
            .change(self.transfer_format)
            .await?;
        let _ = camera
            .binning()
            .await?
            .change(Binning {
                ver: self.binning,
                hor: self.binning,
            })
            .await?;
        let _ = camera.gain().await?.change(self.gain).await?;
        let _ = camera.offset().await?.change(self.offset).await?;

        if let Some(temp) = self.tempurature {
            let _ = camera.temperature().await?.change(temp).await?;
        }
        Ok(())
    }
}

impl Camera {
    async fn get_driver_name(device: &ActiveDevice) -> Result<Text, super::DeviceSelectionError> {
        if let Some(driver_name) = get_parameter_value(device, "DRIVER_INFO", "DRIVER_NAME").await {
            return Ok(driver_name);
        }
        panic!("Err(DeviceSelectionError::DeviceMismatch)")
    }

    fn get_config(driver_name: &Text) -> Result<CameraConfig, super::DeviceSelectionError> {
        match driver_name.value.as_str() {
            "ZWO CCD" => Ok(CameraConfig {
                capture_format: OneOfMany::new(
                    "CCD_CAPTURE_FORMAT",
                    [
                        ("ASI_IMG_RAW8", CaptureFormat::Raw8),
                        ("ASI_IMG_RAW16", CaptureFormat::Raw16),
                    ]
                    .into_iter()
                    .collect(),
                ),
                transfer_format: OneOfMany::new(
                    "CCD_TRANSFER_FORMAT",
                    [
                        ("FORMAT_FITS", TransferFormat::Fits),
                        ("FORMAT_XISF", TransferFormat::Xisf),
                        ("FORMAT_NATIVE", TransferFormat::Native),
                    ]
                    .into_iter()
                    .collect(),
                ),
                cooler: OneOfMany::new(
                    "CCD_COOLER",
                    [("COOLER_ON", Cooler::On), ("COOLER_OFF", Cooler::Off)]
                        .into_iter()
                        .collect(),
                ),
                tempurature: SingleValueParamConfig::new(
                    "CCD_TEMPERATURE",
                    "CCD_TEMPERATURE_VALUE",
                ),
                gain: SingleValueParamConfig::new("CCD_CONTROLS", "Gain"),
                offset: SingleValueParamConfig::new("CCD_CONTROLS", "Offset"),
                image_type: OneOfMany::new(
                    "CCD_FRAME_TYPE",
                    [
                        ("FRAME_FLAT", image_type::ImageType::Flat),
                        ("FRAME_BIAS", image_type::ImageType::Bias),
                        ("FRAME_DARK", image_type::ImageType::Dark),
                        ("FRAME_LIGHT", image_type::ImageType::Light),
                    ]
                    .into_iter()
                    .collect(),
                ),
                binning: BinningConfig::new("CCD_BINNING", "HOR_BIN", "VER_BIN"),
                exposure: SingleValueParamConfig::new("CCD_EXPOSURE", "CCD_EXPOSURE_VALUE"),
                image: SingleValueParamConfig::new("CCD1", "CCD1"),
                abort: SingleValueParamConfig::new("CCD_ABORT_EXPOSURE", "ABORT"),
            }),
            "CCD Simulator" => Ok(CameraConfig {
                capture_format: OneOfMany::new(
                    "CCD_CAPTURE_FORMAT",
                    [("INDI_MONO", CaptureFormat::Raw8)].into_iter().collect(),
                ),
                transfer_format: OneOfMany::new(
                    "CCD_TRANSFER_FORMAT",
                    [
                        ("FORMAT_FITS", TransferFormat::Fits),
                        ("FORMAT_XISF", TransferFormat::Xisf),
                        ("FORMAT_NATIVE", TransferFormat::Native),
                    ]
                    .into_iter()
                    .collect(),
                ),
                cooler: OneOfMany::new(
                    "CCD_COOLER",
                    [("COOLER_ON", Cooler::On), ("COOLER_OFF", Cooler::Off)]
                        .into_iter()
                        .collect(),
                ),
                tempurature: SingleValueParamConfig::new(
                    "CCD_TEMPERATURE",
                    "CCD_TEMPERATURE_VALUE",
                ),
                gain: SingleValueParamConfig::new("CCD_GAIN", "GAIN"),
                offset: SingleValueParamConfig::new("CCD_OFFSET", "OFFSET"),
                image_type: OneOfMany::new(
                    "CCD_FRAME_TYPE",
                    [
                        ("FRAME_FLAT", image_type::ImageType::Flat),
                        ("FRAME_BIAS", image_type::ImageType::Bias),
                        ("FRAME_DARK", image_type::ImageType::Dark),
                        ("FRAME_LIGHT", image_type::ImageType::Light),
                    ]
                    .into_iter()
                    .collect(),
                ),
                binning: BinningConfig::new("CCD_BINNING", "HOR_BIN", "VER_BIN"),
                exposure: SingleValueParamConfig::new("CCD_EXPOSURE", "CCD_EXPOSURE_VALUE"),
                image: SingleValueParamConfig::new("CCD1", "CCD1"),
                abort: SingleValueParamConfig::new("CCD_ABORT_EXPOSURE", "ABORT"),
            }),
            _ => Err(DeviceSelectionError::DeviceMismatch),
        }
    }
    pub async fn new(device: ActiveDevice) -> Result<Self, super::DeviceSelectionError> {
        let driver_name = Self::get_driver_name(&device).await?;

        Ok(Camera {
            device,
            config: Self::get_config(&driver_name)?,
        })
    }

    pub async fn capture_format(&self) -> Result<CaptureFormatParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.capture_format.clone())
                .await?
                .into(),
        )
    }

    pub async fn transfer_format(&self) -> Result<TransferFormatParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.transfer_format.clone())
                .await?
                .into(),
        )
    }

    pub async fn cooler(&self) -> Result<CoolerParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.cooler.clone())
                .await?
                .into(),
        )
    }

    pub async fn temperature(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.tempurature.clone())
                .await?
                .into(),
        )
    }

    pub async fn gain(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.gain.clone())
                .await?
                .into(),
        )
    }

    pub async fn offset(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.offset.clone())
                .await?
                .into(),
        )
    }

    pub async fn image_type(&self) -> Result<ImageTypeParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.image_type.clone())
                .await?
                .into(),
        )
    }

    pub async fn binning(&self) -> Result<BinningParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.binning.clone())
                .await?
                .into(),
        )
    }

    pub async fn exposure(&self) -> Result<NumberParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.exposure.clone())
                .await?
                .into(),
        )
    }

    pub async fn image(&self) -> Result<BlobParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.image.clone())
                .await?
                .into(),
        )
    }

    pub async fn abort(&self) -> Result<SwitchParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.abort.clone())
                .await?
                .into(),
        )
    }

    pub async fn capture_image_from_param(
        &self,
        exposure: Duration,
        image_param: &BlobParameter,
    ) -> Result<Blob, DeviceError> {
        use twinkle_client::OnDropFutureExt;
        let exposure = exposure.as_secs_f64();
        let exposure_param = self.exposure().await?;
        image_param.enable_blob(indi::BlobEnable::Also).await.unwrap();

        let mut image_changes = image_param.changes();
        let mut exposure_changes = exposure_param.changes();

        exposure_param.set(exposure)?;
        // drop(exposure_param);

        let mut previous_exposure_secs = exposure;
        let exposing = Arc::new(Mutex::new(true));
        let exposing_ondrop = exposing.clone();

        let abort = self.abort().await?;

        // Wait for exposure to run out
        wait_fn(
            &mut exposure_changes,
            Duration::from_secs(exposure.ceil() as u64 + 10),
            move |exposure_param| {
                // Exposure goes to idle when canceled
                if *exposure_param.get_state() == PropertyState::Idle {
                    return Err(ChangeError::<()>::Canceled);
                }
                let remaining_exposure: f64 = exposure_param.get().unwrap().value.into();
                // Image is done exposing, new image data should be sent very soon
                if remaining_exposure == 0.0 {
                    *exposing.lock().unwrap() = false;
                    return Ok(notify::Status::Complete(exposure_param));
                }
                // remaining exposure didn't change, nothing to check
                if previous_exposure_secs == remaining_exposure {
                    return Ok(notify::Status::Pending);
                }
                // Make sure exposure changed by a reasonable amount.
                // If another exposure is started before our exposure is finished
                //  there is a good chance the remaining exposure won't have changed
                //  by the amount of time since the last tick.
                let exposure_change = Duration::from_millis(
                    ((previous_exposure_secs - remaining_exposure).abs() * 1000.0) as u64,
                );
                if exposure_change > Duration::from_millis(1100) {
                    return Err(ChangeError::Canceled);
                }
                previous_exposure_secs = remaining_exposure;

                // Nothing funky happened, so we're still waiting for the
                // exposure to finish.
                Ok(notify::Status::Pending)
            },
        )
        .on_drop(|| {
            if *exposing_ondrop.lock().unwrap() {
                tracing::warn!("Canceling exposure");
                if let Err(e) = abort.set(true) {
                    tracing::error!("Error aborting exposure on drop: {:?}", e);
                }
            }
        })
        .await?;

        match image_changes.next().await {
            Some(Ok(image)) => {
                let blob = image.get()?;
                Ok(blob)
            }
            Some(Err(_)) => Err(DeviceError::Missing),
            None => Err(DeviceError::Missing),
        }
    }

    pub async fn pixel_scale(&self) -> f64 {
        let ccd_info = self.device.get_parameter("CCD_INFO").await.unwrap();

        let ccd_binning = self.device.get_parameter("CCD_BINNING").await.unwrap();

        let binning: f64 = {
            let ccd_binning_lock = ccd_binning.read().await;
            ccd_binning_lock
                .get_values::<HashMap<String, Number>>()
                .unwrap()
                .get("HOR_BIN")
                .unwrap()
                .value
                .into()
        };
        let pixel_scale = {
            let ccd_info_lock = ccd_info.read().await;
            let ccd_pixel_size: f64 = ccd_info_lock
                .get_values::<HashMap<String, Number>>()
                .unwrap()
                .get("CCD_PIXEL_SIZE")
                .unwrap()
                .value
                .into();
            binning * ccd_pixel_size / 800.0 * 180.0 / std::f64::consts::PI * 3.6
        };

        pixel_scale
    }
}

#[cfg(test)]
mod test {
    use std::{sync::Arc, time::Duration};

    use tracing_test::traced_test;

    use crate::telescope::{OpticsConfig, Telescope, TelescopeConfig};

    #[tokio::test]
    #[traced_test]
    async fn test_expose() {
        let telescope = Arc::new(
            Telescope::new(
                "indi:7624",
                TelescopeConfig {
                    mount: String::from("Telescope Simulator"),
                    primary_optics: OpticsConfig {
                        focal_length: 800.0,
                        aperture: 203.0,
                    },
                    primary_camera: String::from("CCD Simulator"),
                    focuser: String::from("Focuser Simulator"),
                    filter_wheel: String::from("Filter Simulator"),
                    flat_panel: String::from("Light Panel Simulator"),
                },
            )
            .await,
        );

        let capture_task = tokio::task::spawn({
            let camera = telescope.get_primary_camera().await.unwrap();
            let camera_ccd = telescope.get_primary_camera_ccd().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
            async move {
                let fits_data = camera
                    .capture_image_from_param(Duration::from_secs(1), &camera_ccd)
                    .await
                    .unwrap();

                fits_data.value.unwrap();
            }
        });

        let connect_task = tokio::task::spawn({
            async move {

            }
        });
        tokio::try_join!(capture_task, connect_task).unwrap();
    }
}
