use ndarray::ArrayD;
use std::f64::consts::PI;

use crate::egui::fits_render::Elipse;
use opencv::{
    self,
    core::BORDER_CONSTANT,
    imgproc::{
        morphology_default_border_value, CHAIN_APPROX_NONE, MORPH_ELLIPSE, RETR_LIST, THRESH_BINARY,
    },
    prelude::Mat,
};

use super::{CollimationCalculator, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct DefocusedStar {
    /// Amount to blur.  Real value passed to opencv will be blur*2 + 1
    pub blur: i32,
    pub threshold: f64,
}

impl Default for DefocusedStar {
    fn default() -> Self {
        Self {
            blur: 7,
            threshold: 40.0,
        }
    }
}

impl CollimationCalculator for DefocusedStar {
    fn calculate(&self, data: &ArrayD<u16>) -> Result<Box<dyn Iterator<Item = Elipse>>> {
        let raw_data: Vec<u8> = data.iter().map(|x| (*x >> 8) as u8).collect();
        let image = Mat::from_slice_rows_cols(&raw_data, data.shape()[0], data.shape()[1])?;

        let output: Mat = image.clone();

        let input: Mat = Default::default();
        let (input, mut output) = (output, input);

        opencv::imgproc::median_blur(&input, &mut output, self.blur * 2 + 1).unwrap();
        let (input, mut output) = (output, input);

        opencv::imgproc::threshold(&input, &mut output, self.threshold, 255.0, THRESH_BINARY)?;
        let (input, mut output) = (output, input);

        let kernel = opencv::imgproc::get_structuring_element(
            MORPH_ELLIPSE,
            (5, 5).into(),
            (-1, -1).into(),
        )?;
        opencv::imgproc::morphology_ex(
            &input,
            &mut output,
            opencv::imgproc::MORPH_CLOSE,
            &kernel,
            (-1, -1).into(),
            1,
            BORDER_CONSTANT,
            morphology_default_border_value()?,
        )?;
        let (input, _) = (output, input);

        let mut contours: opencv::core::Vector<opencv::core::Vector<opencv::core::Point>> =
            Default::default();
        opencv::imgproc::find_contours(
            &input,
            &mut contours,
            RETR_LIST,
            CHAIN_APPROX_NONE,
            (0, 0).into(),
        )?;

        return Ok(Box::new(
            contours
                .into_iter()
                .map(|contour| {
                    let m = opencv::imgproc::moments(&contour, false).unwrap();
                    (contour, m)
                })
                .filter(|(_contour, m)| m.m00 != 0.0)
                .flat_map(|(contour, m)| {
                    let area = opencv::imgproc::contour_area(&contour, false).unwrap();
                    let radius = (4.0 * area / PI).sqrt() / 2.0;

                    [
                        Elipse {
                            x: (m.m10 / m.m00) as f32,
                            y: (m.m01 / m.m00) as f32,
                            a: radius as f32,
                            b: radius as f32,
                            theta: 0.0,
                        },
                        Elipse {
                            x: (m.m10 / m.m00) as f32,
                            y: (m.m01 / m.m00) as f32,
                            a: 1.0 as f32,
                            b: 1.0 as f32,
                            theta: 0.0,
                        },
                    ]
                }),
        ));
    }
}
