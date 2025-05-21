use std::{path::Path, sync::Arc, time::Duration};

use indi::{client::Notify, Blob};
use tokio::{fs::File, io::AsyncWriteExt};
use twinkle_api::{analysis::Statistics, fits::AsFits, flats::{Config, FlatRun}};
use twinkle_client::OnDropFutureExt;

use crate::{flats::FlatError, telescope::{camera::{self, CaptureFormat, TransferFormat}, filter_wheel, flat_panel, Connectable, Telescope}};
pub async fn start(telescope: Arc<Telescope>, config: Config, state: Arc<Notify<FlatRun>>) -> Result<(), FlatError> {
    let flat_panel = telescope
        .get_flat_panel()
        .await?;
    
    let light = flat_panel.light().await?;
    light.set(true)?;

    inner_start(telescope, config, state).on_drop(|| {
        if let Err(e) = light.set(false) {
            tracing::error!("Unable to to turn off flatpanel: {:?}", e);
        }
    }).await?;

    Ok(())
}

pub async fn inner_start(telescope: Arc<Telescope>, config: Config, state: Arc<Notify<FlatRun>>) -> Result<(), FlatError> {
    let flat_panel = telescope
        .get_flat_panel()
        .await?;
    let _ = flat_panel.connect().await?;

    let filter_wheel = telescope
        .get_filter_wheel()
        .await?;
    let _ = filter_wheel.connect().await?;

    let camera = telescope
        .get_primary_camera()
        .await?;
    let _ = filter_wheel.connect().await?;

    let fp_config = flat_panel::Config { is_on: true.into() };
    let _ = fp_config.set(&flat_panel).await?;

    let total = config.total_images() as f32;
    let mut completed = 0.;
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

                    match run(
                        telescope.clone(),
                        Duration::from_secs(3),
                        config.adu_target,
                        config.adu_margin,
                    )
                    .await {
                        Ok(blob) => {
                            let filename = Path::new("/storage/calibration/Flats/data");
                            let filename = filename
                                .join(format!("bin_{}", binning))
                                .join(&filter.name);
                            tokio::fs::create_dir_all(&filename).await?;
                            let mut file = File::create(filename.join(format!("Flat_{}_{:02}.fits", filter.name, i))).await?;
                            file.write_all(&blob.value.unwrap()).await?;
                            file.flush().await?;
            
                            completed += 1.;
                            state.write().await.progress = completed / total;
                            break;
                        },
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

pub async fn run(
    telescope: Arc<Telescope>,
    exposure: Duration,
    target_adu: u16,
    margin: u16,
) -> Result<Blob, FlatError> {
    let camera = telescope
        .get_primary_camera()
        .await?;
    let flat_panel = telescope
        .get_flat_panel()
        .await?;

    let fp_brightness = flat_panel.brightness().await?;
    let mut fp_level = fp_brightness.get().await?;

    loop {
        fp_level.value = fp_level.value.max(fp_level.min).min(fp_level.max);
        tracing::info!("Setting panel brightness: {}", f64::from(fp_level.value));
        let _ = fp_brightness
            .change(fp_level.clone())
            .await?;

        tracing::info!("Exposing for {}s", exposure.as_millis() as f64 / 1000f64);
        let fits_data = camera
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
        tracing::info!("flat panel: {}", fp_level.value);

        let target_median = target_adu;
        if target_median.abs_diff(stats.median) <= margin {
            fp_level.value = fp_level.value * (target_median as f64) / (stats.median as f64);
            break Ok(fits_data);
        } else if stats.median as f32 > 0.8 * u16::MAX as f32 {
            fp_level.value = (f64::from(fp_level.value) / 2.0).into();
        } else if (stats.median as f32) < { 0.1 * u16::MAX as f32 } {
            fp_level.value = fp_level.value * 2.0;
        } else {
            fp_level.value = fp_level.value * (target_median as f64) / (stats.median as f64);
        }
    }
}
