use std::{fs::File, io::BufReader, path::PathBuf, time::Duration};

use analysis::Statistics;
use calibration::{CalibrationDescription, Dark, Flat, HasCalibration};
use fitsrs::Fits;
use indi::client::active_device::FitsImage;
use ndarray::{
    array, Array, Array2, ArrayBase, ArrayD, ArrayView, Dim, Dimension, IntoDimension, Ix2, IxDyn,
    IxDynImpl, OwnedRepr, SliceInfo, SliceInfoElem, ViewRepr, Zip,
};
use ndarray_conv::*;

pub mod analysis;
pub mod calibration;
pub mod egui;

pub trait HasImage {
    fn get_data(&self) -> &ArrayD<u16>;
    fn get_data_mut(&mut self) -> &mut ArrayD<u16>;
    fn get_statistics(&self) -> &Statistics;
    fn set_statistics(&mut self, stats: Statistics);
}

pub struct Image {
    data: ArrayD<u16>,
    stats: Statistics,
    flat: calibration::CalibrationDescription,
    dark: calibration::CalibrationDescription,
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
        let data = fits_image.read_image()?;
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

fn convert_vec_to_arrayd(vec: Vec<i16>, shape: &[usize]) -> Result<ArrayD<u16>, &'static str> {
    // Explicitly specify the type for expected_len
    let expected_len: usize = shape.iter().fold(1, |acc, &dim| acc * dim);

    if vec.len() != expected_len {
        return Err("Vector length does not match the specified shape");
    }

    // Convert i16 to u16, handling potential negative values
    let u16_vec: Vec<u16> = vec
        .iter()
        .map(|&x| {
            // let bytes = x.to_ne_bytes(); // Get native-endian bytes
            // let swapped_bytes = [bytes[1], bytes[0]]; // Swap the bytes
            // u16::from_ne_bytes(swapped_bytes)

            (x as i32 - 32768) as u16
            // u16::from_be_bytes(x.to_ne_bytes())
        })
        .collect();

    // Create an ArrayD from the vector and reshape it
    let array = ArrayD::from_shape_vec(IxDyn(shape), u16_vec)
        .map_err(|_| "Failed to create ArrayD with the given shape")?;

    Ok(array)
}

fn read_fits(mut hdu_list: Fits<BufReader<File>>) -> Result<ArrayD<u16>, fitsrs::error::Error> {
    while let Some(Ok(hdu)) = hdu_list.next() {
        if let fitsrs::HDU::Primary(hdu) = hdu {
            let xtension = hdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();
            if let fitsrs::Pixels::I16(it) = hdu_list.get_data(&hdu).pixels() {
                let data: Vec<_> = it.collect();
                return Ok(convert_vec_to_arrayd(
                    data,
                    &[naxis2 as usize, naxis1 as usize],
                )?);
            }
        }
    }
    Err(fitsrs::error::Error::DynamicError(
        "No image data found".to_string(),
    ))
}
impl TryFrom<PathBuf> for Image {
    type Error = fitsio::errors::Error;

    fn try_from(filename: PathBuf) -> Result<Self, Self::Error> {
        let data = read_fits(Fits::from_reader(BufReader::new(
            File::open(&filename).unwrap(),
        )))
        .unwrap();
        // let mut fptr = FitsFile::open(filename)?;
        // let hdu = fptr.primary_hdu()?;
        // let data: ArrayD<u16> = hdu.read_image(&mut fptr)?;

        let stats = Statistics::new(&data.view());
        dbg!(&stats);

        // // let frame: String = hdu.read_key(&mut fptr, "FRAME")?;
        // let flat = CalibrationDescription::Flat(Flat {
        //     filter: hdu.read_key(&mut fptr, "FILTER")?,
        // });

        // let dark = CalibrationDescription::Dark(Dark {
        //     offset: hdu.read_key::<f64>(&mut fptr, "OFFSET")? as i32,
        //     gain: hdu.read_key::<f64>(&mut fptr, "GAIN")? as i32,
        //     exposure: Duration::from_secs(hdu.read_key::<f64>(&mut fptr, "EXPTIME")? as u64),
        // });
        Ok(Image {
            data,
            stats,
            flat: CalibrationDescription::Flat(Flat {
                filter: "Foo".to_string(),
            }),
            dark: CalibrationDescription::Dark(Dark {
                offset: 0,
                gain: 0,
                exposure: Duration::from_secs(0u64),
            }),
        })
    }
}
//fitsrs
// unique: 3034,
// median: 33640,
// mean: 33655.406,
// mad: 84,
// std_dev: 0.06874606,
// clip_high: Sample {
//     value: 65487,
//     count: 1,
// },
// clip_low: Sample {
//     value: 348,
//     count: 1,
// },

// fitsio
// unique: 3034,
// median: 872,
// mean: 889.49994,
// mad: 84,
// std_dev: 0.11451386,
// clip_high: Sample {
//     value: 65531,
//     count: 189,
// },
// clip_low: Sample {
//     value: 548,
//     count: 1,
// },

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
