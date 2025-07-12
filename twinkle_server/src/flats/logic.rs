use std::{path::Path, sync::Arc, time::Duration};

use indi::{client::Notify, Blob};
use tokio::{fs::File, io::AsyncWriteExt};
use twinkle_api::{
    analysis::Statistics,
    fits::AsFits,
    flats::{Config, FlatRun, LightSource},
};

use crate::{
    flats::FlatError,
    telescope::{
        camera::{self, CaptureFormat, ImageType, TransferFormat},
        filter_wheel,
        flat_panel::Light,
        Connectable, Telescope,
    },
};
pub async fn start(
    telescope: Telescope,
    config: Config,
    state: Arc<Notify<FlatRun>>,
) -> Result<(), FlatError> {
    inner_start(telescope, config, state).await?;

    Ok(())
}

struct CaptureState<'a> {
    free: f64,
    min: f64,
    max: f64,
    config: &'a Config,
    flat_panel: Option<Light>,
}

impl<'a> Drop for CaptureState<'a> {
    fn drop(&mut self) {
        if let Some(flat_panel) = &self.flat_panel {
            if let Err(e) = flat_panel.set(false) {
                tracing::error!("Unable to turn flat panel off: {:?}", e);
            }
        }
    }
}

impl<'a> CaptureState<'a> {
    async fn new(telescope: &Telescope, config: &'a Config) -> Result<Self, FlatError> {
        Ok(match &config.light_source {
            LightSource::FlatPanel(_) => {
                let flat_panel = telescope.get_flat_panel().await?;
                let _ = flat_panel.connect().await?;
                let fp_brightness = flat_panel.brightness().await?;
                let fp_level = fp_brightness.get().await?;
                let light = flat_panel.light().await?;
                light.set(true)?;

                CaptureState {
                    free: f64::from(fp_level.value),
                    min: fp_level.min,
                    max: fp_level.max,
                    config,
                    flat_panel: Some(light),
                }
            }
            LightSource::Sky {
                min_exposure,
                max_exposure,
            } => CaptureState {
                free: min_exposure.as_secs_f64(),
                min: min_exposure.as_secs_f64(),
                max: max_exposure.as_secs_f64(),
                config,
                flat_panel: None,
            },
        })
    }

    async fn capture(&mut self, telescope: &Telescope) -> Result<Blob, FlatError> {
        let exposure: Duration = match &self.config.light_source {
            LightSource::FlatPanel(duration) => {
                tracing::info!("Setting panel brightness: {}", f64::from(self.free));
                let flat_panel = telescope.get_flat_panel().await?;
                let fp_brightness = flat_panel.brightness().await?;

                let _ = fp_brightness.change(self.free).await?;
                *duration
            }
            LightSource::Sky {
                min_exposure: _,
                max_exposure: _,
            } => Duration::from_secs_f64(f64::from(self.free)),
        };

        tracing::info!("Exposing for {}s", exposure.as_millis() as f64 / 1000f64);
        let fits_data = telescope
            .get_primary_camera()
            .await?
            .capture_image(exposure)
            .await?;

        let stats = {
            let image_data = fits_data
                .value
                .as_ref()
                .ok_or(FlatError::MissingBlob)?
                .as_fits()
                .read_image()?;
            Statistics::new(&image_data.view())
        };

        tracing::info!("median adu: {}", &stats.median);

        let target_median = self.config.adu_target;
        if target_median.abs_diff(stats.median) <= self.config.adu_margin {
            self.set_free(self.free * (target_median as f64) / (stats.median as f64));
            return Ok(fits_data);
        } else if stats.median as f32 > 0.9 * u16::MAX as f32 {
            self.set_free((f64::from(self.free) / 2.0).into());
        } else if (stats.median as f32) < { 0.1 * u16::MAX as f32 } {
            self.set_free(dbg!(self.free * 2.0));
        } else {
            self.set_free(self.free * (target_median as f64) / (stats.median as f64));
        }
        Err(FlatError::BadAdu)
    }

    fn set_free(&mut self, new_free: f64) {
        self.free = new_free.max(self.min).min(self.max);
    }
}

async fn inner_start(
    telescope: Telescope,
    config: Config,
    state: Arc<Notify<FlatRun>>,
) -> Result<(), FlatError> {
    let filter_wheel = telescope.get_filter_wheel().await?;

    let camera = telescope.get_primary_camera().await?;
    let _ = camera.image_type().await?.change(ImageType::Flat).await?;

    let total = config.total_images() as f32;
    let mut completed = 0.;

    let mut capture_state = CaptureState::new(&telescope, &config).await?;

    for filter in config.filters.iter().filter_map(
        |(filter, enabled)| {
            if *enabled {
                Some(filter)
            } else {
                None
            }
        },
    ) {
        tracing::info!("Starting for filter: {:?}", filter);
        let filter_config = filter_wheel::Config {
            filter: filter.clone().into(),
        };
        filter_config.set(&filter_wheel).await?;

        for binning in
            config.binnings.iter().filter_map(
                |(binning, enabled)| {
                    if *enabled {
                        Some(binning)
                    } else {
                        None
                    }
                },
            )
        {
            tracing::info!("Starting for binning: {:?}", binning);
            let camera_config = camera::Config {
                bit_depth: CaptureFormat::Raw16,
                transfer_format: TransferFormat::Fits,
                image_type: camera::ImageType::Flat,
                binning: *binning,
                gain: config.gain,
                offset: config.offset,
                tempurature: None,
            };
            let _ = camera_config.set(&camera).await?;
            for i in 0..config.count {
                tracing::info!("Creating: {}/bin{}/{}", filter.name, binning, i);
                loop {
                    match capture_state.capture(&telescope).await {
                        Ok(blob) => {
                            let filename = Path::new("/storage/calibration/Flats/data");
                            let filename =
                                filename.join(format!("bin_{}", binning)).join(&filter.name);
                            tokio::fs::create_dir_all(&filename).await?;
                            let mut file = File::create(
                                filename.join(format!("Flat_{}_{:02}.fits", filter.name, i)),
                            )
                            .await?;
                            file.write_all(&blob.value.unwrap()).await?;
                            file.flush().await?;

                            completed += 1.;
                            state.write().await.progress = completed / total;
                            break;
                        }
                        Err(e) => {
                            tracing::error!("Unable to get frame: {:?}", e);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
