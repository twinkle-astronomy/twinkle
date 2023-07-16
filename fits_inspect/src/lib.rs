use std::{time::Duration, path::PathBuf, sync::Arc};

use analysis::Statistics;
use calibration::{HasCalibration, Flat, Dark, CalibrationDescription};
use fitsio::FitsFile;
use indi::client::device::FitsImage;
use ndarray::{
    array, Array, Array2, ArrayBase, ArrayD, ArrayView, Dim, Dimension, IntoDimension, Ix2, IxDyn,
    IxDynImpl, OwnedRepr, SliceInfo, SliceInfoElem, ViewRepr, Zip,
};
use ndarray_conv::*;

pub mod egui;
pub mod analysis;
pub mod calibration;




pub trait HasImage {
    fn get_data(&self) -> Arc<ArrayD<u16>>;
    fn get_data_mut(&mut self) -> &mut ArrayD<u16>;
    fn get_statistics(&self) -> &Statistics;
    fn set_statistics(&mut self, stats: Statistics);
}

pub struct Image {
    data: Arc<ArrayD<u16>>,
    stats: Statistics,
    flat: calibration::CalibrationDescription,
    dark: calibration::CalibrationDescription,
}

impl HasImage for Image {
    fn get_data(&self) -> Arc<ArrayD<u16>> {
        self.data.clone()
    }

    fn get_data_mut(&mut self) -> &mut ArrayD<u16> {
        Arc::make_mut(&mut self.data)
    }

    fn get_statistics(&self) -> &Statistics {
        &self.stats
    }
    fn set_statistics(&mut self, stats: Statistics) {
        self.stats = stats;
    }

}

impl HasCalibration for Image {
    fn describe_flat(&self) -> &CalibrationDescription {
        &self.flat
    }
    fn describe_dark(&self) -> &CalibrationDescription {
        &self.dark
    }
}

impl TryFrom<FitsImage> for Image {
    type Error = fitsio::errors::Error;

    fn try_from(fits_image: FitsImage) -> Result<Self, Self::Error> {
        let data = Arc::new(fits_image.read_image()?);
        let stats = Statistics::new(&data.view());
        let flat = CalibrationDescription::Flat(Flat {
            filter: fits_image.read_header("FILTER")?,
        });
        let dark = CalibrationDescription::Dark(Dark {
            offset: fits_image.read_header("OFFSET")?,
            gain: fits_image.read_header("GAIN")?,
            exposure: Duration::from_secs(fits_image.read_header::<i32>("EXPTIME")? as u64),
        });
        
        Ok(Image {
            data,
            stats,
            flat,
            dark,
        })
    }
}

impl TryFrom<PathBuf> for Image {
    type Error = fitsio::errors::Error;

    fn try_from(filename: PathBuf) -> Result<Self, Self::Error> {

        let mut fptr = FitsFile::open(filename)?;

        let hdu = fptr.primary_hdu()?;
        let data: Arc<ArrayD<u16>> = Arc::new(hdu.read_image(&mut fptr)?);
        let stats = Statistics::new(&data.view());

        // let frame: String = hdu.read_key(&mut fptr, "FRAME")?;
        let flat = CalibrationDescription::Flat(Flat {
            filter: hdu.read_key(&mut fptr, "FILTER")?
        });

        let dark = CalibrationDescription::Dark(Dark {
            offset: hdu.read_key::<f64>(&mut fptr, "OFFSET")? as i32,
            gain: hdu.read_key::<f64>(&mut fptr, "GAIN")? as i32,
            exposure: Duration::from_secs(hdu.read_key::<f64>(&mut fptr, "EXPTIME")? as u64),
        });
        Ok(Image {
            data,
            stats,
            flat,
            dark,
        })
    }
}

pub fn phd2_convolve(data: &ArrayD<u16>) -> Array2<f32> {
    let data_f32: ArrayBase<OwnedRepr<f32>, Ix2> = data
        .map(|element| f32::from(*element))
        .into_dimensionality::<Ix2>()
        .unwrap();
    const A: f32 = 0.906;
    const B1: f32 = 0.584;
    const B2: f32 = 0.365;
    const C1: f32 = 0.117;
    const C2: f32 = 0.049;
    const C3: f32 = -0.05;
    const D1: f32 = -0.064;
    const D2: f32 = -0.074;
    const D3: f32 = -0.094;
    let kernel = array![
        [D3, D3, D3, D3, D3, D3, D3, D3, D3],
        [D3, D3, D3, D2, D1, D2, D3, D3, D3],
        [D3, D3, C3, C2, C1, C2, C3, D3, D3],
        [D3, D2, C2, B2, B1, B2, C2, D2, D3],
        [D3, D1, C1, B1, A, B1, C1, D1, D3],
        [D3, D2, C2, B2, B1, B2, C2, D2, D3],
        [D3, D3, C3, C2, C1, C2, C3, D3, D3],
        [D3, D3, D3, D2, D1, D2, D3, D3, D3],
        [D3, D3, D3, D3, D3, D3, D3, D3, D3],
    ];

    data_f32.conv_2d_fft(&kernel).unwrap()
}

pub fn sobel(data: &ArrayD<u16>) -> Array2<f32> {
    let z: ArrayBase<OwnedRepr<f32>, Ix2> = data
        .mapv(|element| f32::from(element))
        .into_dimensionality::<Ix2>()
        .unwrap();

    let g_x = array![[-1., 0., 1.], [-2., 0., 2.], [-1., 0., 1.]];
    let g_y = array![[-1., -2., -1.], [0., 0., 0.], [1., 2., 1.]];

    let mut data_gx = z.conv_2d_fft(&g_x).unwrap();
    let data_gy = z.conv_2d_fft(&g_y).unwrap();

    Zip::from(&mut data_gx)
        .and(&data_gy)
        .for_each(|gx, &gy| *gx = ((*gx) * (*gx) + gy * gy).sqrt());

    data_gx
}

pub trait Windowed<T: Copy + Sync + Send> {
    fn padded(&self, edge_padding: ndarray::IxDyn, padding_value: T) -> Self;
    fn map_window<E, F, U>(&self, padding_value: T, window: E, function: F) -> ArrayD<U>
    where
        E: IntoDimension<Dim = IxDyn>,
        F: Sync + Send + Fn(ArrayView<T, IxDyn>) -> U,
        U: Sync + Send + Copy + num_traits::identities::Zero;
}

impl<T: Copy + Sync + Send> Windowed<T> for ArrayD<T> {
    fn padded(&self, edge_padding: ndarray::IxDyn, padding_value: T) -> Self {
        let outer_padding: Vec<usize> =
            edge_padding.as_array_view().iter().map(|x| x * 2).collect();
        let padded_dim = Dim(self.shape()) + Dim(outer_padding);

        let mut data_padded: ArrayBase<OwnedRepr<T>, IxDyn> =
            Array::from_elem(padded_dim, padding_value);

        let slice: Vec<SliceInfoElem> = edge_padding
            .as_array_view()
            .iter()
            .map(|x| {
                let r = ndarray::SliceInfoElem::Slice {
                    start: *x as isize,
                    end: Some(-(*x as isize)),
                    step: 1,
                };
                r
            })
            .collect();

        let slice_info: SliceInfo<&[_], IxDyn, IxDyn> =
            unsafe { ndarray::SliceInfo::new(&slice[..]).expect("") };

        let mut sliced_data: ArrayBase<ViewRepr<&mut T>, Dim<IxDynImpl>> =
            data_padded.slice_mut(slice_info);
        Zip::from(&mut sliced_data)
            .and(self)
            .par_for_each(|lhs, rhs| {
                *lhs = *rhs;
            });
        data_padded
    }

    fn map_window<E, F, U>(&self, padding_value: T, window: E, function: F) -> ArrayD<U>
    where
        E: IntoDimension<Dim = IxDyn>,
        F: Sync + Send + Fn(ArrayView<T, IxDyn>) -> U,
        U: Sync + Send + Copy + num_traits::identities::Zero,
    {
        let mut result = ArrayD::<U>::zeros(self.shape());
        let window_dimension = window.into_dimension();

        let padding = window_dimension
            .as_array_view()
            .iter()
            .map(|x| x / 2)
            .collect::<Vec<usize>>();
        let data_padded = self.padded(Dim(padding.clone()), padding_value);

        Zip::from(&mut result)
            .and(data_padded.windows(window_dimension))
            .par_for_each(|x, window| {
                *x = function(window);
            });

        return result;
    }
}
