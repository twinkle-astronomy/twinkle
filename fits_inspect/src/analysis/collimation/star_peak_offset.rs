use crate::{analysis::Statistics, egui::fits_render::Elipse};

use super::{CollimationCalculator, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct StarPeakOffset {
    pub threshold: f32,
}

impl Default for StarPeakOffset {
    fn default() -> Self {
        Self { threshold: 2. }
    }
}

impl CollimationCalculator for StarPeakOffset {
    fn calculate(
        &self,
        image: &ndarray::ArrayD<u16>,
    ) -> Result<Box<dyn Iterator<Item = crate::egui::fits_render::Elipse>>> {
        let stats = Statistics::new(&image.view());
        let sep_image = crate::analysis::sep::Image::new(image)?;

        let stars: Vec<crate::analysis::sep::CatalogEntry> = sep_image
            .extract(Some(self.threshold))
            .unwrap_or(vec![])
            .into_iter()
            .filter(|x| x.flag == 0)
            .filter(|x| x.peak * 1.2 < stats.clip_high.value as f32)
            .collect();

        let mut star_iter = stars.iter();
        let ((x, y), (xpeak, ypeak)) = if let Some(first) = star_iter.next() {
            star_iter.fold(
                (
                    (first.x, first.y),
                    (first.xcpeak as f64, first.ycpeak as f64),
                ),
                |((x, y), (xpeak, ypeak)), star| {
                    (
                        (x + star.x, y + star.y),
                        (xpeak + star.xcpeak as f64, ypeak + star.ycpeak as f64),
                    )
                },
            )
        } else {
            return Ok(Box::new(vec![].into_iter()));
        };
        let ((x, y), (xpeak, ypeak)) = (
            (x / stars.len() as f64, y / stars.len() as f64),
            (xpeak / stars.len() as f64, ypeak / stars.len() as f64),
        );

        let centers = [
            Elipse {
                x: x as f32,
                y: y as f32,
                a: 0.5,
                b: 0.5,
                theta: 0.0,
            },
            Elipse {
                x: x as f32,
                y: y as f32,
                a: 0.5,
                b: 10.5,
                theta: 0.0,
            },
            Elipse {
                x: xpeak as f32,
                y: ypeak as f32,
                a: 0.5,
                b: 0.5,
                theta: 0.0,
            },
            Elipse {
                x: xpeak as f32,
                y: ypeak as f32,
                a: 10.5,
                b: 0.5,
                theta: 0.0,
            },
        ];

        let stars = stars
            .into_iter()
            .flat_map(|x| {
                let center1 = Elipse {
                    x: x.x as f32,
                    y: x.y as f32,
                    a: 0.5,
                    b: 0.5,
                    theta: 0.0,
                };
                let center2 = Elipse {
                    x: x.xpeak as f32,
                    y: x.ypeak as f32,
                    a: 0.5,
                    b: 0.5,
                    theta: 0.0,
                };
                [x.into(), center1, center2]
            })
            .chain(centers);
        Ok(Box::new(stars))
    }
}
