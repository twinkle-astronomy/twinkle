use crate::{analysis::Statistics, HasImage};
use fitsio::FitsFile;
use ndarray::{ArrayD, Zip};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub enum CalibrationDescription {
    Flat(Flat),
    Dark(Dark),
}

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct Flat {
    pub filter: String,
}

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct Dark {
    pub offset: i32,
    pub gain: i32,
    pub exposure: Duration,
}

pub type CalibrationStore<T> = HashMap<CalibrationDescription, T>;

pub enum Error {
    MissingFLat,
    MissingDark,
}

pub trait HasCalibration {
    fn describe_flat(&self) -> &CalibrationDescription;
    fn describe_dark(&self) -> &CalibrationDescription;
}

pub trait CanCalibrate {
    fn calibrate<T: HasImage>(&mut self, dark: &T, flat: &T) -> Result<&Self, Error>
    where
        Self: Sized;
}

impl<T: HasCalibration + HasImage> CanCalibrate for T {
    fn calibrate<I: HasImage>(&mut self, dark: &I, flat: &I) -> Result<&Self, Error>
    where
        Self: Sized,
    {
        let data = self.get_data_mut();

        let flat_median = flat.get_statistics().median as f32;
        dbg!(flat_median);

        let mut clipped = 0;
        Zip::from(data)
            .and(flat.get_data())
            .and(dark.get_data())
            .for_each(|data, &flat, &dark| {
                *data = if *data > dark {
                    *data - dark
                } else {
                    clipped += 1;
                    0
                };

                let flat_factor = flat_median / (flat as f32);
                *data = (*data as f32 * flat_factor) as u16
            });
        dbg!(clipped);

        let data = self.get_data();
        self.set_statistics(Statistics::new(&data.view()));
        Ok(self)
    }
}

pub struct Image {
    data: ArrayD<u16>,
    stats: Statistics,
    pub desc: CalibrationDescription,
}

impl HasImage for Image {
    fn get_data(&self) -> &ArrayD<u16> {
        &self.data
    }

    fn get_data_mut(&mut self) -> &mut ArrayD<u16> {
        &mut self.data
    }

    fn get_statistics(&self) -> &Statistics {
        &self.stats
    }

    fn set_statistics(&mut self, stats: Statistics) {
        self.stats = stats;
    }
}

#[derive(Debug)]
pub enum ImageError {
    FitsError(fitsio::errors::Error),
    InvalidFrame(String),
}

impl From<fitsio::errors::Error> for ImageError {
    fn from(value: fitsio::errors::Error) -> Self {
        ImageError::FitsError(value)
    }
}

impl TryFrom<PathBuf> for Image {
    type Error = ImageError;

    fn try_from(filename: PathBuf) -> Result<Self, Self::Error> {
        let mut fptr = FitsFile::open(filename)?;

        let hdu = fptr.primary_hdu()?;
        let data: ArrayD<u16> = hdu.read_image(&mut fptr)?;
        let stats = Statistics::new(&data.view());

        let frame: String = hdu.read_key(&mut fptr, "FRAME")?;
        let desc = match frame.to_uppercase().as_str() {
            "FLAT" => CalibrationDescription::Flat(Flat {
                filter: hdu.read_key(&mut fptr, "FILTER")?,
            }),
            "DARK" => CalibrationDescription::Dark(Dark {
                offset: hdu.read_key(&mut fptr, "OFFSET")?,
                gain: hdu.read_key(&mut fptr, "GAIN")?,
                exposure: Duration::from_secs(hdu.read_key::<i32>(&mut fptr, "EXPTIME")? as u64),
            }),
            frame => return Err(ImageError::InvalidFrame(String::from(frame))),
        };
        Ok(Image { data, stats, desc })
    }
}
