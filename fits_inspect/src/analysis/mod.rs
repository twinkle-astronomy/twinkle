mod statistics;
pub use statistics::*;
pub mod sep;

pub mod astigmatism;

use ndarray::Array;
use ndarray_stats::CorrelationExt;
use rmpfit::{MPFitter, MPResult};
use std::cmp::Ordering;

pub trait Star {
    fn image_center(&self) -> [f64; 2];

    fn intensity_peak(&self) -> f32;
    fn intensity_loc(&self) -> [usize; 2];

    fn flux(&self) -> f32;

    fn fwhm(&self) -> f32;
}

#[derive(Debug)]
pub enum MPError {
    /// General input parameter error
    Input,
    /// User function produced non-finite values
    Nan,
    /// No user data points were supplied
    Empty,
    /// No free parameters
    NoFree,
    /// Initial values inconsistent with constraints
    InitBounds,
    /// Initial constraints inconsistent
    Bounds,
    /// Not enough degrees of freedom
    DoF,
    /// Error during evaluation by user
    Eval,
}

#[derive(Debug)]
pub enum FitError {
    MPError(MPError),
    NoMin,
}

impl From<rmpfit::MPError> for FitError {
    fn from(err: rmpfit::MPError) -> Self {
        match err {
            rmpfit::MPError::Input => FitError::MPError(MPError::Input),
            rmpfit::MPError::Nan => FitError::MPError(MPError::Nan),
            rmpfit::MPError::Empty => FitError::MPError(MPError::Empty),
            rmpfit::MPError::NoFree => FitError::MPError(MPError::NoFree),
            rmpfit::MPError::InitBounds => FitError::MPError(MPError::InitBounds),
            rmpfit::MPError::Bounds => FitError::MPError(MPError::Bounds),
            rmpfit::MPError::DoF => FitError::MPError(MPError::DoF),
            rmpfit::MPError::Eval => FitError::MPError(MPError::Eval),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HyperbolicFit {
    // Hyperbolic curve parameters
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,

    // Pearson correlation coefficient
    pub pcc: f64,
}

impl HyperbolicFit {
    pub fn new(points: &Vec<[f64; 2]>) -> Result<Self, FitError> {
        let min_x = points.iter().min_by(|[x1, _y1], [x2, _y2]| {
            if x1 < x2 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });

        let min_elem = match min_x {
            Some(elem) => elem,
            None => return Err(FitError::NoMin),
        };

        let min_c = min_elem[0];
        let min_d = min_elem[1];
        let mut params = [1.0, 1.0, min_c, min_d];
        let samples = HyperbolicPoints { points };
        // let mut fit = HyperbolicFit {
        //     samples: HyperbolicPoints { points },
        //     params: [1.0, 1.0, min_c, min_d]
        // };
        samples.mpfit(&mut params, None, &Default::default())?;
        let pcc = samples.coor(&params);
        Ok(HyperbolicFit {
            a: params[0],
            b: params[1],
            c: params[2],
            d: params[3],
            pcc,
        })
    }

    pub fn expected_y(&self, x: f64) -> f64 {
        HyperbolicPoints::expected_y(x, &[self.a, self.b, self.c, self.d])
    }

    pub fn middle_x(&self) -> f64 {
        self.c
    }
}
struct HyperbolicPoints<'a> {
    points: &'a Vec<[f64; 2]>,
}

impl<'a> HyperbolicPoints<'a> {
    fn coor(&self, params: &[f64]) -> f64 {
        let expected = Array::from_iter(
            self.points
                .iter()
                .map(|[x, _y]| Self::expected_y(*x, params)),
        );

        let measured = Array::from_iter(self.points.iter().map(|[_x, y]| *y));

        let mut a = Array::zeros((0, measured.len()));
        a.push_row(expected.view())
            .expect("Adding expected values row");
        a.push_row(measured.view())
            .expect("Adding measured values row");
        a.pearson_correlation().unwrap()[(0, 1)]
    }

    fn expected_y(x: f64, params: &[f64]) -> f64 {
        let a = params[0];
        let b = params[1];
        let c = params[2];
        let d = params[3];

        (((x - c) * (x - c) / (a * a) + 1.0) * b * b).sqrt() + d
    }
}
impl<'a> MPFitter for HyperbolicPoints<'a> {
    fn eval(&self, params: &[f64], deviates: &mut [f64]) -> MPResult<()> {
        for (deviate, [x, y]) in deviates.iter_mut().zip(self.points) {
            *deviate = y - Self::expected_y(*x, params);
        }
        Ok(())
    }

    fn number_of_points(&self) -> usize {
        self.points.len()
    }
}
#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    #[test]
    fn test_foo() {
        let points = vec![
            [26881.0, 6.77],
            [26893.0, 6.24],
            [26905.0, 5.77],
            [26917.0, 5.10],
            [26929.0, 4.83],
            [26941.0, 4.50],
            [26944.0, 4.40],
            [26953.0, 4.80],
            [26965.0, 5.19],
            [26977.0, 5.28],
            [26989.0, 5.87],
            [27001.0, 6.36],
        ];

        let fit = HyperbolicFit::new(&points).unwrap();

        assert_relative_eq!(fit.c, 26944.0, epsilon = 1.0);
        assert_relative_eq!(fit.d, 3.89, epsilon = 0.01);
    }
}
