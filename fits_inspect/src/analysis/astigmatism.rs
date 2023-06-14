use ndarray::ArrayD;

use super::sep;

trait WeightedValue {
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

fn weighted_mean<T: WeightedValue>(values: impl std::iter::Iterator<Item = T>) -> f64 {
    let mut total = 0 as f64;
    let mut weights = 0 as f64;
    for item in values {
        total += item.value() * item.weight();
        weights += item.weight();
    }

    total / weights
}

pub fn focus(data: ArrayD<u16>) -> f64 {
    // let mut fptr = fitsio::FitsFile::open(filename).expect("Opening fits file");
    // let hdu = fptr.primary_hdu().expect("Getting primary HDU");
    // let data: ArrayD<u16> = hdu.read_image(&mut fptr).expect("reading image");
    // dbg!(data.shape());

    let center_x = data.shape()[1] as f64;
    let center_y = data.shape()[0] as f64;

    let mut sep_image = sep::Image::new(data).unwrap();
    let bkg = sep_image.background().unwrap();
    sep_image.sub(&bkg).unwrap();
    let catalog = sep_image.extract(None).unwrap();

    let f = catalog.iter().map(|star| {
        let x = center_x - star.x;
        let y = center_y - star.y;
        let position_theta = y.atan2(x);

        let delta = (position_theta - star.theta as f64).abs();
        let delta = if delta > std::f64::consts::PI / 2.0 {
            std::f64::consts::PI - delta
        } else {
            delta
        };
        // println!("************************************");
        // dbg!(star.theta, star.theta as f64 * 180.0 / std::f64::consts::PI);
        // dbg!(position_theta, position_theta * 180.0 / std::f64::consts::PI);
        // dbg!((star.x, star.y));

        // dbg!((star.a as f64 / star.b as f64, delta * 180.0 / std::f64::consts::PI));

        (delta, 1.0) //star.a as f64 / star.b as f64 - 1.0)
    });

    weighted_mean(f) * 180.0 / std::f64::consts::PI - 45.0 //(45.0 * std::f64::consts::PI / 180.0)
}
