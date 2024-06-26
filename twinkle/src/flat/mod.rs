use crate::{Action, Telescope};
use fits_inspect::analysis::Statistics;
use indi::{client::device::FitsImage, SwitchState};
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use twinkle_client::notify::Notify;

#[derive(Debug, Clone)]
pub struct Config {
    pub filter: String,
    pub adu_target: u16,
    pub adu_margin: u16,
    pub binning: f64,
    pub gain: f64,
    pub offset: f64,
    pub exposure: Duration,
    pub fp_level: f64,
}

#[derive(Debug, Clone)]
pub struct SetConfig {
    pub count: usize,
    pub filters: HashMap<String, bool>,
    pub adu_target: u16,
    pub adu_margin: u16,
    pub binnings: HashMap<u8, bool>,
    pub gain: f64,
    pub offset: f64,
    pub exposure: Duration,
}

impl SetConfig {
    pub fn expected_total(&self) -> usize {
        self.count
            * self.binnings.iter().filter(|(_k, v)| **v).count()
            * self.filters.iter().filter(|(_k, v)| **v).count()
    }
}

#[derive(Clone, Debug)]
pub struct Status {
    pub image: Option<Arc<FitsImage>>,
    pub complete: u32,
}
impl Default for Status {
    fn default() -> Self {
        Status {
            image: None,
            complete: 0,
        }
    }
}
pub struct Runner {
    status: Arc<Notify<Status>>,
    pub task: JoinHandle<()>,
}

impl Runner {
    pub fn new(config: Config, telescope: Arc<Telescope>) -> Runner {
        let status = Arc::new(Notify::new(Status::default()));

        let task_status = status.clone();
        let task = tokio::spawn(async move {
            let (_compl, _duration) = Runner::run(&task_status, config, telescope).await;
        });

        Runner { status, task }
    }

    pub fn new_set(config: SetConfig, telescope: Arc<Telescope>) -> Runner {
        let status = Arc::new(Notify::new(Status::default()));

        let task_status = status.clone();
        let task = tokio::spawn(async move {
            let mut fp_level = 100.0;
            for (filter, _) in config.filters.iter().filter(|(_k, v)| **v) {
                for (bin, _) in config.binnings.iter().filter(|(_k, v)| **v) {
                    for i in 1..=config.count {
                        let config = Config {
                            filter: filter.clone(),
                            adu_target: config.adu_target,
                            adu_margin: config.adu_margin,
                            binning: *bin as f64,
                            gain: config.gain,
                            offset: config.offset,
                            exposure: config.exposure,
                            fp_level,
                        };
                        let (fits, next_fp_level) =
                            Runner::run(&task_status, config, telescope.clone()).await;
                        fp_level = next_fp_level;
                        let root = telescope.root_path();
                        let filename = Path::new(&root);
                        let filename = filename
                            .join(format!("bin_{}", bin))
                            .join(filter)
                            .join(format!("Flat_{}_{:02}.fits", filter, i));
                        fits.save(filename).expect("Saving image");
                        {
                            let mut lock = task_status.lock().unwrap();
                            lock.complete += 1;
                        }
                    }
                }
            }
            telescope
                .get_flat_panel()
                .await
                .expect("Getting flat panel")
                .change(
                    "FLAT_LIGHT_CONTROL",
                    vec![("FLAT_LIGHT_ON", SwitchState::Off)],
                )
                .await
                .expect("Turning off FP");
        });

        Runner { status, task }
    }

    async fn run(
        status: &Arc<Notify<Status>>,
        config: Config,
        telescope: Arc<Telescope>,
    ) -> (Arc<FitsImage>, f64) {
        let camera = telescope
            .get_primary_camera()
            .await
            .expect("Getting camera");
        let filter_wheel = telescope
            .get_filter_wheel()
            .await
            .expect("Getting filter wheel");

        let flat_panel = telescope
            .get_flat_panel()
            .await
            .expect("Getting flat panel");

        tokio::try_join!(
            camera.change("CONNECTION", vec![("CONNECT", true)]),
            filter_wheel.change("CONNECTION", vec![("CONNECT", true)]),
            flat_panel.change("CONNECTION", vec![("CONNECT", true)]),
        )
        .expect("Connecting to devices");

        flat_panel
            .change(
                "FLAT_LIGHT_CONTROL",
                vec![("FLAT_LIGHT_ON", SwitchState::On)],
            )
            .await
            .expect("Setting brightness");

        let camera_ccd = telescope
            .get_primary_camera_ccd()
            .await
            .expect("Getting camera ccd");

        tokio::try_join!(
            camera.change("CCD_CAPTURE_FORMAT", vec![("ASI_IMG_RAW16", true)]),
            camera.change("CCD_TRANSFER_FORMAT", vec![("FORMAT_FITS", true)]),
            camera.change(
                "CCD_CONTROLS",
                vec![("Offset", config.offset), ("Gain", config.gain)]
            ),
            // camera.change("FITS_HEADER", vec![("FITS_OBJECT", "")]),
            camera.change(
                "CCD_BINNING",
                vec![("HOR_BIN", config.binning), ("VER_BIN", config.binning)]
            ),
            camera.change("CCD_FRAME_TYPE", vec![("FRAME_FLAT", true)]),
            filter_wheel.change_filter(&config.filter)
        )
        .expect("Configuring camera");

        let mut fp_level = config.fp_level;

        loop {
            fp_level = fp_level.max(0.0).min(1000.0);
            println!("Setting panel brightness: {}", fp_level);
            flat_panel
                .change(
                    "FLAT_LIGHT_INTENSITY",
                    vec![("FLAT_LIGHT_INTENSITY_VALUE", fp_level)],
                )
                .await
                .expect("Setting brightness");

            println!(
                "Exposing for {}s",
                config.exposure.as_millis() as f64 / 1000f64
            );
            let fits_data = camera
                .capture_image_from_param(config.exposure, &camera_ccd)
                .await
                .expect("Capturing image");

            let image_data = fits_data.read_image().expect("Reading captured image");
            print!("Analyzing...");
            let stats = Statistics::new(&image_data.view());

            let fits_data = Arc::new(fits_data);
            {
                let mut lock = status.lock().unwrap();
                lock.image = Some(fits_data.clone());
            }
            println!(" median adu: {}", &stats.median);

            let target_median = config.adu_target;
            if target_median.abs_diff(stats.median) <= config.adu_margin {
                fp_level = (target_median as f64) / (stats.median as f64) * fp_level;
                println!("Finished getting flat");
                break (fits_data, fp_level);
            } else if stats.median as f32 > 0.8 * u16::MAX as f32 {
                println!("halving");
                fp_level = fp_level / 2.0;
            } else if (stats.median as f32) < { 0.1 * u16::MAX as f32 } {
                println!("Doubling");
                fp_level = fp_level * 2.0;
            } else {
                println!("adjusting");

                fp_level = (target_median as f64) / (stats.median as f64) * fp_level;
            }
        }
    }
}

impl Action<Status> for Runner {
    fn status(&self) -> BroadcastStream<std::sync::Arc<Status>> {
        self.status.subscribe().unwrap()
    }
}
