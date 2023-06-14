extern crate fitsio;

use fits_inspect::analysis::{astigmatism, sep};
use fitsio::images::{ImageDescription, ImageType};
use ndarray::{prelude::*, IxDynImpl};
use std::{env, fs, path::Path};

pub trait WeightedValue {
    fn weight(&self) -> f64;
    fn value(&self) -> f64;
}

impl WeightedValue for (f64, f64) {
    fn weight(&self) -> f64 {
        self.1
    }

    fn value(&self) -> f64 {
        self.0
    }
}

pub fn weighted_mean<T: WeightedValue>(values: impl std::iter::Iterator<Item = T>) -> f64 {
    let mut total = 0 as f64;
    let mut weights = 0 as f64;
    for item in values {
        total += item.value() * item.weight();
        weights += item.weight();
    }

    total / weights
}

fn focal(filename: &String) -> f64 {
    let mut fptr = fitsio::FitsFile::open(filename).expect("Opening fits file");
    let hdu = fptr.primary_hdu().expect("Getting primary HDU");
    let data: ArrayD<u16> = hdu.read_image(&mut fptr).expect("reading image");
    // dbg!(data.shape());
    return astigmatism::focus(data);
}

pub fn lines(filename: &String) {
    let file = Path::new(filename).file_name().unwrap();
    let dir = Path::new(filename).parent().unwrap().join("lines");
    fs::create_dir_all(&dir).unwrap();
    let target_file = dir.join(Path::new(file));
    fs::remove_file(&target_file).ok();
    dbg!(&target_file);

    let mut fptr = fitsio::FitsFile::open(filename).expect("Opening fits file");
    let hdu = fptr.primary_hdu().expect("Getting primary HDU");
    let mut data: ArrayD<u16> = hdu.read_image(&mut fptr).expect("reading image");
    // dbg!(data.shape());

    let mut lines: ArrayD<u16> = Array::zeros(data.shape());

    let sep_image = sep::Image::new(data.clone()).unwrap();
    let bkg = sep_image.background().unwrap();
    let mut catalog = sep_image.extract(None).unwrap();
    catalog.sort_by(|a, b| (-a.a / a.b).partial_cmp(&(-b.a / b.b)).unwrap());
    for star in &catalog {
        // if (star.a as f64 / star.b as f64) < 1.5 {
        //     continue
        // }
        let theta = star.theta;
        // dbg!(theta as f64* std::f64::consts::PI / 180.0);
        //star.y = starx * m + b;
        //stary - star.x * m = b;
        let m = theta.tan() as f64;
        // let b = star.y - star.x * m;
        for x in 0..data.shape()[1] {
            let y = m * (x as f64 - star.x) + star.y;
            if y >= 0.0 && y < data.shape()[0] as f64 {
                lines[&[y as usize, x][..]] += 1;
            }
        }
    }

    let intersections: Vec<Dim<IxDynImpl>> = lines
        .indexed_iter()
        .filter(|(_pos, val)| **val > 2)
        .map(|(pos, _val)| pos)
        .collect();

    if intersections.len() > 0 {
        let x = intersections.iter().map(|dim| dim[1]).sum::<usize>() / intersections.len();
        let y = intersections.iter().map(|dim| dim[0]).sum::<usize>() / intersections.len();

        for x in x - 100..x + 100 {
            data[&[y, x][..]] = std::u16::MAX;
        }

        for y in y - 100..y + 100 {
            data[&[y, x][..]] = std::u16::MAX;
        }
    } else {
        println!("No intersections");
    }

    let image_description = ImageDescription {
        data_type: ImageType::UnsignedShort,
        dimensions: data.shape(),
    };
    let mut fptr = fitsio::FitsFile::create(target_file)
        .with_custom_primary(&image_description)
        .open()
        .unwrap();
    // let mut new_file = fitsio::FitsFile::create(target_file).open().unwrap();

    let hdu = fptr.primary_hdu().unwrap(); //.create_image("EXTNAME".to_string(), &image_description).unwrap();
    let image_data = data.into_raw_vec();
    hdu.write_image(&mut fptr, &image_data).unwrap();

    println!("done");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    for filename in &args[1..] {
        dbg!(filename, focal(filename));
    }
}
