use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use binning::{BinningConfig, BinningParameter};
use capture_format::CaptureFormatParameter;
use cooler::CoolerParameter;
use futures::Stream;
use image_type::ImageTypeParameter;
use indi::{
    client::{active_device::ActiveDevice, ChangeError}, Blob, Number, Parameter, PropertyState, Switch, Text
};
use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt};
use transfer_format::TransferFormatParameter;
use twinkle_client::{notify::ArcCounter, OnDropFutureExt};
use twinkle_client::timeout;

use super::{
    parameter_with_config::{
        get_parameter_value, ActiveParameterWithConfig, BlobParameter, NumberParameter, OneOfMany,
        SingleValueParamConfig, SwitchParameter,
    }, Connectable, DeviceError, DeviceSelectionError
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
    ccd: BlobParameter,
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
    pub async fn new(
        device: ActiveDevice,
        ccd_device: ActiveDevice,
    ) -> Result<Self, super::DeviceSelectionError> {
        let driver_name = Self::get_driver_name(&device).await?;
        let config = Self::get_config(&driver_name)?;

        let ccd = Camera::image(&ccd_device, config.image.clone()).await?;

        ccd.enable_blob(indi::BlobEnable::Also).await?;
        Ok(Camera {
            device,
            config,
            ccd,
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

    pub async fn image(
        device: &ActiveDevice,
        config: SingleValueParamConfig<Blob>,
    ) -> Result<BlobParameter, DeviceError> {
        Ok(ActiveParameterWithConfig::new(&device, config)
            .await?
            .into())
    }

    pub async fn abort(&self) -> Result<SwitchParameter, DeviceError> {
        Ok(
            ActiveParameterWithConfig::new(&self.device, self.config.abort.clone())
                .await?
                .into(),
        )
    }

    #[tracing::instrument(skip(self))]
    pub async fn capture_image(&self, exposure: Duration) -> Result<Blob, DeviceError> {
        let exposure = exposure.as_secs_f64();
        let exposure_param = self.exposure().await?;

        let mut image_changes = self.ccd.changes();
        let mut exposure_changes = exposure_param.changes();

        exposure_param.set(exposure)?;

        let exposing = Arc::new(Mutex::new(true));
        let exposing_ondrop = exposing.clone();

        let abort = self.abort().await?;

        timeout(
            Duration::from_secs(exposure.ceil() as u64 + 10),
            async move {
                let mut started = false;
                while let Some(exposure_param) = exposure_changes.try_next().await? {
                    let remaining_exposure: f64 = exposure_param.get().unwrap().value.into();
                    if !started {
                        if remaining_exposure == exposure {
                            started = true;
                        }
                        continue;
                    }
                    // Image is done exposing, new image data should be sent very soon
                    if remaining_exposure == 0.0 {
                        *exposing.lock().unwrap() = false;
                        if *exposure_param.get_state() == PropertyState::Idle {
                            tracing::error!("Detected external abort");
                            return Err(DeviceError::Missing);
                        }

                        break;
                    }
                }

                match image_changes.next().await {
                    Some(Ok(image)) => {
                        tracing::info!("Got image");
                        Ok(image.get()?)
                    }
                    Some(Err(e)) => {
                        tracing::info!("Error getting image: {:?}", e);
                        Err(DeviceError::Missing)
                    }
                    None => {
                        tracing::info!("Missing image");
                        Err(DeviceError::Missing)
                    }
                }
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
        .await?
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


impl Connectable for Camera {
    async fn connect(
        &self,
    ) -> Result<
        impl Stream<Item = Result<ArcCounter<Parameter>, BroadcastStreamRecvError>>,
        ChangeError<()>,
    > {
        self.device.change("CONNECTION", vec![("CONNECT", true)]).await
    }
}

#[cfg(test)]
mod test {
    use std::{sync::Arc, time::Duration};

    use tokio::time::Instant;
    use tracing_test::traced_test;

    use crate::telescope::{Telescope, TelescopeConfig};

    #[tokio::test]
    #[traced_test]
    async fn test_expose() {
        let mut telescope = 
            Telescope::new(
                TelescopeConfig {
                    mount: String::from("Telescope Simulator"),
                    primary_camera: String::from("CCD Simulator"),
                    focuser: String::from("Focuser Simulator"),
                    filter_wheel: String::from("Filter Simulator"),
                    flat_panel: String::from("Light Panel Simulator"),
                },
            );
        telescope.connect("indi:7624").await;

        let camera = telescope.get_primary_camera().await.unwrap();
        for _ in 0..5 {
            tokio::time::timeout(Duration::from_millis(1500), async {
                let now = Instant::now();
                let fits_data = camera
                    .capture_image(Duration::from_secs_f32(0.01))
                    .await
                    .unwrap();

                fits_data.value.unwrap();
                tracing::info!("got image: {:?}", now.elapsed());
            })
            .await
            .unwrap();
        }
    }
}
