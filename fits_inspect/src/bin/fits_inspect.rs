extern crate fitsio;

use fits_inspect::analysis::Statistics;
use ndarray::prelude::*;
use std::env;

use csv::Writer;

fn main() {
    let args: Vec<String> = env::args().collect();

    let filename = &args[1];

    let file_data = std::fs::read(filename).unwrap();
    let data: ArrayD<u16> = indi::client::device::Device::image_from_fits(&file_data);

    let nd_stats = Statistics::new(&data.view());

    println!("unique: {}", nd_stats.unique);
    println!("mean: {}", nd_stats.mean);
    println!("mad: {}", nd_stats.mad);
    println!("median: {}", nd_stats.median);
    println!(
        "clip_low: {:?}",
        (nd_stats.clip_low.value, nd_stats.clip_low.count)
    );
    println!(
        "clip_high: {:?}",
        (nd_stats.clip_high.value, nd_stats.clip_high.count)
    );

    let mut sep_image =
        fits_inspect::analysis::sep::Image::new(data).expect("Unable to create sep image");
    let bkg = sep_image.background().unwrap();
    bkg.subarray(&mut sep_image)
        .expect("Background subtraction failed");

    let catalog = sep_image.extract(&bkg).expect("Failed to extract features");

    let mut wtr = Writer::from_path("foo.csv").unwrap();
    for entry in catalog {
        wtr.serialize(entry).expect("Serialization failure");
    }

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
