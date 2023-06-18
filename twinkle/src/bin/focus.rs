use fits_inspect::analysis::sep::CatalogEntry;
use fits_inspect::analysis::{sep, HyperbolicFit, Star, Statistics};
use indi::*;
use std::time::Duration;
use std::{collections::HashMap, env};
use twinkle::*;

pub struct FocusMeasurement {
    stars: Vec<CatalogEntry>,
    focuser_position: f64,
}

#[derive(Default)]
pub struct AutoFocus {
    measurements: Vec<FocusMeasurement>,
    model: Option<fits_inspect::analysis::HyperbolicFit>,
}

impl AutoFocus {
    pub fn add(&mut self, measurement: FocusMeasurement) {
        self.measurements.push(measurement);
        if self.measurements.len() >= 4 {
            let data: Vec<[f64; 2]> = self
                .measurements
                .iter()
                .map(|measurement| {
                    [
                        measurement.focuser_position,
                        measurement
                            .stars
                            .iter()
                            .map(|e| e.fwhm() as f64)
                            .sum::<f64>()
                            / measurement.stars.len() as f64,
                    ]
                })
                .collect();

            match HyperbolicFit::new(&data) {
                Ok(model) => {
                    self.model = Some(model);
                }
                Err(e) => {
                    dbg!(e);
                }
            }
        }
    }

    pub fn predicted_focus_position(&self) -> Option<f64> {
        self.model.as_ref().and_then(|model| Some(model.middle_x()))
    }

    pub fn is_complete(&self) -> bool {
        self.measurements.len() > 7 && self.model.is_some()
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let addr = &args[1];

    let config = TelescopeConfig {
        mount: String::from("EQMod Mount"),
        primary_optics: OpticsConfig {
            focal_length: 800.0,
            aperture: 203.0,
        },
        primary_camera: String::from("ZWO CCD ASI294MM Pro"),
        focuser: String::from("ASI EAF"),
        filter_wheel: String::from("ASI EFW"),
    };

    let telescope = Telescope::new(addr, config);

    let camera = telescope
        .get_primary_camera()
        .await
        .expect("getting camera");
    let ccd = telescope.get_primary_camera_ccd().await.unwrap();

    let focuser = telescope.get_focuser().await.expect("Getting focuser");

    let focuser_position: f64 = focuser
        .get_parameter("ABS_FOCUS_POSITION")
        .await
        .unwrap()
        .lock()
        .unwrap()
        .get_values::<HashMap<String, Number>>()
        .unwrap()
        .get("FOCUS_ABSOLUTE_POSITION")
        .unwrap()
        .value
        .into();

    let focus_config = AutoFocusConfig {
        exposure: Duration::from_secs(1),
        filter: String::from("Luminance"),
        step: -10.0,
        start_position: focuser_position + 10.0 * 7.0,
    };
    let mut autofocus = AutoFocus::default();

    focuser
        .change(
            "ABS_FOCUS_POSITION",
            vec![("FOCUS_ABSOLUTE_POSITION", focus_config.start_position)],
        )
        .await
        .unwrap();

    while !autofocus.is_complete() {
        let focuser_position: f64 = focuser
            .get_parameter("ABS_FOCUS_POSITION")
            .await
            .unwrap()
            .lock()
            .unwrap()
            .get_values::<HashMap<String, Number>>()
            .unwrap()
            .get("FOCUS_ABSOLUTE_POSITION")
            .unwrap()
            .value
            .into();

        let fits_data = camera
            .capture_image_from_param(focus_config.exposure, &ccd)
            .await
            .unwrap();
        let image_data = fits_data.read_image().expect("Reading captured image");

        println!("Analyzing...");
        let stats = Statistics::new(&image_data.view());
        dbg!(stats.unique, (stats.unique as f64).log2());

        let mut sep_image = sep::Image::new(image_data).unwrap();
        let bkg = sep_image.background().unwrap();
        sep_image.sub(&bkg).unwrap();
        let catalog = sep_image.extract(None).unwrap();

        autofocus.add(FocusMeasurement {
            focuser_position,
            stars: catalog,
        });
    }

    focuser
        .change(
            "ABS_FOCUS_POSITION",
            vec![(
                "FOCUS_ABSOLUTE_POSITION",
                autofocus.predicted_focus_position().unwrap(),
            )],
        )
        .await
        .unwrap();
}
