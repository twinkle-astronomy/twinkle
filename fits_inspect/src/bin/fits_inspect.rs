extern crate fitsio;

use fits_inspect::*;
use fitsio::{
    images::{ImageDescription, ImageType},
    FitsFile,
};
use ndarray::{prelude::*, FoldWhile, SliceInfo, SliceInfoElem, Zip, IntoDimension};
use std::{env, ffi::c_void};

fn main() {
    let args: Vec<String> = env::args().collect();

    let filename = &args[1];

    let mut fptr = FitsFile::open("images/PSF.fit").unwrap();
    let hdu = fptr.primary_hdu().unwrap();
    let psf: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();
    let psf_i32 = psf.map(|x| i32::from(*x));

    let mut fptr = FitsFile::open(filename).unwrap();
    let hdu = fptr.primary_hdu().unwrap();
    let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

    let mut data_f32: ArrayD<f32> = data.map(|x| *x as f32);

    let nd_stats = Statistics::new(&data.view());

    println!("unique: {}", nd_stats.unique);
    println!("mean: {}", nd_stats.mean);
    println!("median: {}", nd_stats.median);
    println!(
        "clip_low: {:?}",
        (nd_stats.clip_low.value, nd_stats.clip_low.count)
    );
    println!(
        "clip_high: {:?}",
        (nd_stats.clip_high.value, nd_stats.clip_high.count)
    );

    let sep_image = sep_sys::sep_image {
        data: data_f32.as_ptr() as *const c_void,
        noise: std::ptr::null(),
        mask: std::ptr::null(),
        segmap: std::ptr::null(),
        dtype: sep_sys::SEP_TFLOAT,
        ndtype: sep_sys::SEP_TFLOAT,
        mdtype: sep_sys::SEP_TFLOAT,
        sdtype: sep_sys::SEP_TFLOAT,
        w: data_f32.shape()[1] as i32,
        h: data_f32.shape()[0] as i32,
        noiseval: 0.0 as f64,
        noise_type: sep_sys::SEP_NOISE_NONE,
        gain: 0.0 as f64,
        maskthresh: 0.0 as f64,
    };

    unsafe {
        let mut background: *mut sep_sys::sep_bkg = std::ptr::null_mut();
        sep_sys::sep_background(&sep_image, 128, 128, 9, 9, 0.0 as f64, &mut background);
        dbg!(sep_sys::sep_bkg_subarray(
            background,
            data_f32.as_mut_ptr() as *mut c_void,
            sep_sys::SEP_TFLOAT,
        ));
        sep_sys::sep_bkg_free(background);

        let mut catalog: *mut sep_sys::sep_catalog = std::ptr::null_mut();
        dbg!(sep_sys::sep_extract(
            &sep_image,
            800.0,
            sep_sys::SEP_THRESH_ABS,
            10,
            &[
                1.0 as f32, 2.0 as f32, 1.0 as f32, 2.0 as f32, 4.0 as f32, 2.0 as f32, 1.0 as f32,
                2.0 as f32, 1.0 as f32,
            ] as *const f32,
            3,
            3,
            sep_sys::SEP_FILTER_CONV,
            32,
            0.005 as f64,
            1,
            1.0 as f64,
            &mut catalog,
        ));

        let x_mins = std::slice::from_raw_parts((*catalog).xmin, (*catalog).nobj as usize);
        let x_maxs = std::slice::from_raw_parts((*catalog).xmax, (*catalog).nobj as usize);
        let y_mins = std::slice::from_raw_parts((*catalog).ymin, (*catalog).nobj as usize);
        let y_maxs = std::slice::from_raw_parts((*catalog).ymax, (*catalog).nobj as usize);


        let npix = std::slice::from_raw_parts((*catalog).npix, (*catalog).nobj as usize);
        let flux = std::slice::from_raw_parts((*catalog).flux, (*catalog).nobj as usize);
        let thresh = std::slice::from_raw_parts((*catalog).thresh, (*catalog).nobj as usize);
        let peak = std::slice::from_raw_parts((*catalog).peak, (*catalog).nobj as usize);
        
        let mut mask = ArrayD::from_elem(data.shape(), 0 as u16);
        for i in 0..(*catalog).nobj as usize {
            let x = (y_maxs[i] + y_mins[i]) / 2;
            let y = (x_maxs[i] + x_mins[i]) / 2;
            dbg!(
                    (
                        x,
                        y,
                        npix[i],
                        peak[i]
                    )
            );
            for y in y_mins[i]..y_maxs[i] {
                for x in x_mins[i]..=x_maxs[i] {
                    mask[&[y as usize, x as usize][..]] = std::u16::MAX / 2;
                }
            }
        }
        dbg!(*catalog);
        sep_sys::sep_catalog_free(catalog);
   

        // let mut fptr = FitsFile::create("images/mask.fits").open().unwrap();
        // let image_description = ImageDescription {
        //     data_type: ImageType::Short,
        //     dimensions: &mask.shape(),
        // };

        // let hdu = fptr
        //     .create_image("EXTNAME".to_string(), &image_description)
        //     .unwrap();

        // hdu.write_image(&mut fptr, &mask.into_raw_vec())
        //     .unwrap();
    }
    // const A: i32 = (0.906 * u16::MAX as f32) as i32;
    // const B1: i32 = (0.584 * u16::MAX as f32) as i32;
    // const B2: i32 =( 0.365 * u16::MAX as f32) as i32;
    // const C1: i32 =( 0.117* u16::MAX as f32) as i32;
    // const C2: i32 =( 0.049* u16::MAX as f32) as i32;
    // const C3: i32 = (-0.05* u16::MAX as f32) as i32;
    // const D1: i32 = (-0.064* u16::MAX as f32) as i32;
    // const D2: i32 = (-0.074* u16::MAX as f32) as i32;
    // const D3: i32 = (-0.094* u16::MAX as f32) as i32;
    // let kernel = array![
    //     [D3, D3, D3, D3, D3, D3, D3, D3, D3],
    //     [D3, D3, D3, D2, D1, D2, D3, D3, D3],
    //     [D3, D3, C3, C2, C1, C2, C3, D3, D3],
    //     [D3, D2, C2, B2, B1, B2, C2, D2, D3],
    //     [D3, D1, C1, B1, A, B1, C1, D1, D3],
    //     [D3, D2, C2, B2, B1, B2, C2, D2, D3],
    //     [D3, D3, C3, C2, C1, C2, C3, D3, D3],
    //     [D3, D3, D3, D2, D1, D2, D3, D3, D3],
    //     [D3, D3, D3, D3, D3, D3, D3, D3, D3],
    // ];

    // 3 in 9  (00, 01, 02, 03, 04, 05, 06, 07, 08)
    {
        // let window_shape= psf.shape();

        // let psf_shape = window_shape
        //     .iter()
        //     .zip(psf.shape().iter())
        //     .map(|(w, p)| (w / 2 - p / 2, p));
        // let psf_slice: Vec<SliceInfoElem> = psf_shape
        //     .map(|(s, e)| {
        //         let start = s as isize;
        //         let end = Some(start + (*e as isize));
        //         let step = 1;
        //         let r = ndarray::SliceInfoElem::Slice {
        //             start,
        //             end,
        //             step,
        //         };
        //         r
        //     })
        //     .collect();
        // let psf_slice_info: SliceInfo<&[_], IxDyn, IxDyn> =
        //     unsafe { ndarray::SliceInfo::new(&psf_slice[..]).expect("") };

        // let loss = data.map_window(
        //     nd_stats.median,
        //     window_shape, |window| {
        //         // let mut window_vec: Vec<u16> = window.iter().map(|x| *x).collect();
        //         // window_vec.sort_unstable();
        //         // window_vec[window_vec.len() / 2]
        //         // let window_stats = Statistics::new(&window);
        //         let max: u16 = *window.iter().max().unwrap();
        //         let min: u16 = *window.iter().min().unwrap();

        //         //window_stats.median
        //         // let max = window_stats.clip_high.value;
        //         let scale = (max - min) as f32 / std::u16::MAX as f32;

        //         let zipped = Zip::from(&psf).and(&window).fold_while(0 as f64, |acc, p, w| {
        //             let w = ((w - min) as f32 * scale) as u16;
        //             let diff =  p.abs_diff(w) as f64;
        //             let acc = acc + diff;
        //             if acc < 9814181500.0{
        //                 FoldWhile::Continue(acc)
        //             } else {
        //                 FoldWhile::Continue(acc)
        //             }

        //         });

        //         let sum_squares = match zipped  {
        //             FoldWhile::Done(acc) => acc,
        //             FoldWhile::Continue(acc) => acc
        //         };
        //         // dbg!(sum_squares)
        //         (sum_squares / psf.len() as f64) as u16
        //         // let diff = &psf
        //         //     -
        //         //     &window.map(|x| u16::from((((*x - min) as f32)*scale )as u16));

        //         // let loss_sum = diff.map(|x| {
        //         //     let fx:f32 = *x as f32;
        //         //     fx*fx
        //         // }).sum().sqrt();
        //         // // dbg!(loss_sum);
        //         // loss_sum
        // });

        // let mut fptr = FitsFile::create("images/loss.fits").open().unwrap();
        // let image_description = ImageDescription {
        //     data_type: ImageType::Short,
        //     dimensions: &loss.shape(),
        // };

        // let hdu = fptr
        //     .create_image("EXTNAME".to_string(), &image_description)
        //     .unwrap();

        // hdu.write_image(&mut fptr, &loss.into_raw_vec())
        //     .unwrap();
    }
    // {
    //     let convolved = phd2_convolve(&data);

    //     let mut fptr = FitsFile::create("images/convolved.fits").open().unwrap();
    //     let image_description = ImageDescription {
    //         data_type: ImageType::Float,
    //         dimensions: &convolved.shape(),
    //     };

    //     let hdu = fptr
    //         .create_image("EXTNAME".to_string(), &image_description)
    //         .unwrap();

    //     hdu.write_image(&mut fptr, &convolved.into_raw_vec())
    //         .unwrap();
    // }
    // {
    //     let sobeled = sobel(&data);

    //     let mut fptr = FitsFile::create("images/sobel.fits").open().unwrap();
    //     let image_description = ImageDescription {
    //         data_type: ImageType::Float,
    //         dimensions: &sobeled.shape(),
    //     };

    //     let hdu = fptr
    //         .create_image("EXTNAME".to_string(), &image_description)
    //         .unwrap();

    //     hdu.write_image(&mut fptr, &sobeled.into_raw_vec()).unwrap();
    // }
}
