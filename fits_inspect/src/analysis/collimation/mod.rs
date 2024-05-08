// mod defocused_star;
// pub use defocused_star::*;

mod star_peak_offset;
pub use star_peak_offset::*;

use ndarray::ArrayD;

use crate::egui::fits_render::Elipse;

#[derive(Debug)]
pub enum Error {
    // OpencvError(opencv::Error),
    ShapeError(ndarray::ShapeError),
    SepApiStatus(super::sep::SepApiStatus),
}

// impl From<opencv::Error> for Error {
//     fn from(value: opencv::Error) -> Self {
//         Error::OpencvError(value)
//     }
// }

impl From<ndarray::ShapeError> for Error {
    fn from(value: ndarray::ShapeError) -> Self {
        Error::ShapeError(value)
    }
}

impl From<super::sep::SepApiStatus> for Error {
    fn from(value: super::sep::SepApiStatus) -> Self {
        Error::SepApiStatus(value)
    }
}

type Result<T> = std::result::Result<T, Error>;

pub trait CollimationCalculator {
    fn calculate(&self, data: &ArrayD<u16>) -> Result<Box<dyn Iterator<Item = Elipse>>>;
}
