use ndarray::{Array2, ArrayD};
use sep_sys;
use serde::Serialize;
use std::ffi::c_void;

use super::Star;

#[derive(PartialEq, Debug)]
pub enum SepApiStatus {
    ReturnOk,
    MemoryAllocError,
    PixstackFull,
    IllegalDtype,
    IllegalSubpix,
    NonEllipseParams,
    IllegalAperParams,
    DeblendOverflow,
    LineNotInBuf,
    RelthresNoNoise,
    UnknownNoiseType,
    UnknownSepError,
}

impl From<i32> for SepApiStatus {
    fn from(value: i32) -> Self {
        match value {
            0 => SepApiStatus::ReturnOk,
            1 => SepApiStatus::MemoryAllocError,
            2 => SepApiStatus::PixstackFull,
            3 => SepApiStatus::IllegalDtype,
            4 => SepApiStatus::IllegalSubpix,
            5 => SepApiStatus::NonEllipseParams,
            6 => SepApiStatus::IllegalAperParams,
            7 => SepApiStatus::DeblendOverflow,
            8 => SepApiStatus::LineNotInBuf,
            9 => SepApiStatus::RelthresNoNoise,
            10 => SepApiStatus::UnknownNoiseType,
            _ => SepApiStatus::UnknownSepError,
        }
    }
}

pub struct Image {
    sep_sys_image: sep_sys::sep_image,
    image: Array2<f32>,
}

impl<'a> Image {
    pub fn new(image: ArrayD<u16>) -> Result<Image, ndarray::ShapeError> {
        let data_f32: Array2<f32> = image.into_dimensionality()?.map(|x| *x as f32);
        let data_ptr = data_f32.as_slice().unwrap();
        let sep_image = sep_sys::sep_image {
            data: data_ptr.as_ptr() as *const c_void,
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

        Ok(Image {
            sep_sys_image: sep_image,
            image: data_f32,
        })
    }

    pub fn background(&self) -> Result<Background, SepApiStatus> {
        let mut background = Background {
            sep_sys_background: std::ptr::null_mut(),
        };
        let status: SepApiStatus = unsafe {
            sep_sys::sep_background(
                &self.sep_sys_image,
                128,
                128,
                9,
                9,
                0.0 as f64,
                &mut background.sep_sys_background,
            )
        }
        .into();

        match status {
            SepApiStatus::ReturnOk => Ok(background),
            error => Err(error),
        }
    }

    pub fn extract(&self, _background: &Background) -> Result<Vec<CatalogEntry>, SepApiStatus> {
        let mut catalog: *mut sep_sys::sep_catalog = std::ptr::null_mut();
        let status: SepApiStatus = unsafe {
            sep_sys::sep_extract(
                &self.sep_sys_image,
                (2.0 as f32).powf(15.0),
                sep_sys::SEP_THRESH_ABS,
                10,
                std::ptr::null(),
                3,
                3,
                sep_sys::SEP_FILTER_CONV,
                32,
                0.005 as f64,
                1,
                1.0 as f64,
                &mut catalog,
            )
        }
        .into();

        if SepApiStatus::ReturnOk != status {
            return Err(status);
        }

        let nobj: usize = unsafe { *catalog }.nobj as usize;
        let mut catalog_vec = Vec::with_capacity(nobj);

        for i in 0..nobj {
            let a = unsafe { std::slice::from_raw_parts((*catalog).a, nobj) }[i];
            let b = unsafe { std::slice::from_raw_parts((*catalog).b, nobj) }[i];
            catalog_vec.push(CatalogEntry {
                thresh: unsafe { std::slice::from_raw_parts((*catalog).thresh, nobj) }[i],
                npix: unsafe { std::slice::from_raw_parts((*catalog).npix, nobj) }[i],
                tnpix: unsafe { std::slice::from_raw_parts((*catalog).tnpix, nobj) }[i],
                xmin: unsafe { std::slice::from_raw_parts((*catalog).xmin, nobj) }[i],
                xmax: unsafe { std::slice::from_raw_parts((*catalog).xmax, nobj) }[i],
                ymin: unsafe { std::slice::from_raw_parts((*catalog).ymin, nobj) }[i],
                ymax: unsafe { std::slice::from_raw_parts((*catalog).ymax, nobj) }[i],
                x: unsafe { std::slice::from_raw_parts((*catalog).x, nobj) }[i],
                y: unsafe { std::slice::from_raw_parts((*catalog).y, nobj) }[i],
                x2: unsafe { std::slice::from_raw_parts((*catalog).x2, nobj) }[i],
                y2: unsafe { std::slice::from_raw_parts((*catalog).y2, nobj) }[i],
                xy: unsafe { std::slice::from_raw_parts((*catalog).xy, nobj) }[i],
                a: a,
                b: b,
                theta: unsafe { std::slice::from_raw_parts((*catalog).theta, nobj) }[i],
                cxx: unsafe { std::slice::from_raw_parts((*catalog).cxx, nobj) }[i],
                cyy: unsafe { std::slice::from_raw_parts((*catalog).cyy, nobj) }[i],
                cxy: unsafe { std::slice::from_raw_parts((*catalog).cxy, nobj) }[i],
                cflux: unsafe { std::slice::from_raw_parts((*catalog).cflux, nobj) }[i],
                flux: unsafe { std::slice::from_raw_parts((*catalog).flux, nobj) }[i],
                cpeak: unsafe { std::slice::from_raw_parts((*catalog).cpeak, nobj) }[i],
                peak: unsafe { std::slice::from_raw_parts((*catalog).peak, nobj) }[i],
                xcpeak: unsafe { std::slice::from_raw_parts((*catalog).xcpeak, nobj) }[i],
                ycpeak: unsafe { std::slice::from_raw_parts((*catalog).ycpeak, nobj) }[i],
                xpeak: unsafe { std::slice::from_raw_parts((*catalog).xpeak, nobj) }[i],
                ypeak: unsafe { std::slice::from_raw_parts((*catalog).ypeak, nobj) }[i],
                flag: unsafe { std::slice::from_raw_parts((*catalog).flag, nobj) }[i],
            })
        }
        unsafe {
            sep_sys::sep_catalog_free(catalog);
        }
        Ok(catalog_vec)
    }
}

#[derive(Serialize, Debug)]
pub struct CatalogEntry {
    pub thresh: f32,
    pub npix: i32,
    pub tnpix: i32,

    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,

    pub x: f64,
    pub y: f64,
    pub x2: f64,
    pub y2: f64,
    pub xy: f64,

    pub a: f32,
    pub b: f32,
    pub theta: f32,
    pub cxx: f32,
    pub cyy: f32,
    pub cxy: f32,

    pub cflux: f32,
    pub flux: f32,

    pub cpeak: f32,
    pub peak: f32,
    pub xcpeak: i32,
    pub ycpeak: i32,
    pub xpeak: i32,
    pub ypeak: i32,
    pub flag: i16,
}

impl Star for CatalogEntry {
    fn image_center(&self) -> [f64; 2] {
        [self.x, self.y]
    }

    fn intensity_peak(&self) -> f32 {
        self.peak
    }

    fn intensity_loc(&self) -> [usize; 2] {
        [self.xpeak as usize, self.ypeak as usize]
    }

    fn flux(&self) -> f32 {
        self.flux
    }

    fn fwhm(&self) -> f32 {
        2.0 * std::f32::consts::LN_2 * (self.a * self.a + self.b * self.b).sqrt()
    }
}

pub struct Background {
    sep_sys_background: *mut sep_sys::sep_bkg,
}

impl Background {
    pub fn global(&self) -> f32 {
        unsafe { *self.sep_sys_background }.global
    }
    pub fn globalrms(&self) -> f32 {
        unsafe { *self.sep_sys_background }.globalrms
    }

    pub fn subarray(&self, image: &mut Image) -> Result<(), SepApiStatus> {
        let status: SepApiStatus = unsafe {
            sep_sys::sep_bkg_subarray(
                self.sep_sys_background,
                image.image.as_ptr() as *mut c_void,
                sep_sys::SEP_TFLOAT,
            )
        }
        .into();

        match status {
            SepApiStatus::ReturnOk => Ok(()),
            error => Err(error),
        }
    }
}
impl Drop for Background {
    fn drop(&mut self) {
        if self.sep_sys_background != std::ptr::null_mut() {
            unsafe {
                sep_sys::sep_bkg_free(self.sep_sys_background);
            }
            self.sep_sys_background = std::ptr::null_mut();
        }
    }
}
