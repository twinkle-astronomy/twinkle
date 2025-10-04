use serde::{Deserialize, Serialize};
use std::io::Cursor;

use fitsrs::Fits;
use ndarray::{ArrayD, IxDyn};

pub trait AsFits {
    fn as_fits(&'_ self) -> FitsImage<'_>;
}

impl AsFits for Vec<u8> {
    fn as_fits(&'_ self) -> FitsImage<'_> {
        FitsImage::new(self.as_slice())
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use core::arch::wasm32::*;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub fn convert_bytes(bytes: &[u8], bzero: u16) -> Vec<u16> {
    let len = bytes.len() / 2;
    let mut result: Vec<u16> = Vec::with_capacity(len);
    unsafe {
        result.set_len(len);
    }

    // Process 8 i16 values (16 bytes) at a time
    let simd_len = len & !7; // Round down to multiple of 8

    unsafe {
        // Since 32768 doesn't fit in i16, we'll use the minimum value (-32768)
        // and adjust our calculations accordingly
        let min_i16 = i16x8_splat(u16::cast_signed(bzero));
        let result_ptr = result.as_mut_ptr();

        // Create a buffer for our shuffle mask
        let shuffle_bytes: [u8; 16] = [1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14];
        let shuffle_mask = v128_load(shuffle_bytes.as_ptr() as *const v128);

        for i in (0..simd_len).step_by(8) {
            // Load 16 bytes
            let src_ptr = bytes.as_ptr().add(i * 2);
            let v = v128_load(src_ptr as *const v128);

            // Swap bytes for big endian to little endian conversion
            let be_corrected = i8x16_swizzle(v, shuffle_mask);

            // Instead of subtracting 32768, we'll add the minimum value
            // This is equivalent to: (x - 32768) = (x + (-32768) - 655=36)
            // For a signed 16-bit integer, wrapping around is what we want
            let values = i16x8_add(be_corrected, min_i16);

            // Store result as u16
            v128_store(result_ptr.add(i) as *mut v128, values);
        }

        // Handle remaining elements
        for i in simd_len..len {
            let j = i * 2;
            if j + 1 < bytes.len() {
                // Using a different approach: converting to u16 directly
                // This should give the same result as (x as i32 - 32768) as u16
                let x = u16::from_be_bytes([bytes[j], bytes[j + 1]]);
                // x is already a u16, so we just need to adjust the value
                *result_ptr.add(i) = x.wrapping_sub(32768);
            }
        }
    }
    result
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub fn convert_bytes(bytes: &[u8], bzero: u16) -> Vec<u16> {
    let len = bytes.len() / 2;
    let mut result: Vec<u16> = Vec::with_capacity(len);

    // Use unsafe for speed - avoid bounds checking
    unsafe {
        result.set_len(len);
        let src_ptr = bytes.as_ptr();
        let dst_ptr = result.as_mut_ptr();

        for i in 0..len {
            let idx = i * 2;
            let x = i16::from_be_bytes([*src_ptr.add(idx), *src_ptr.add(idx + 1)]);
            *dst_ptr.add(i) = (x as i32 - bzero as i32) as u16;
        }
    }
    result
}

fn read_fits(mut hdu_list: Fits<Cursor<&[u8]>>) -> Result<ArrayD<u16>, fitsrs::error::Error> {
    while let Some(Ok(hdu)) = hdu_list.next() {
        if let fitsrs::HDU::Primary(hdu) = hdu {
            let header = hdu.get_header();
            let bzero = header.get_parsed::<i64>("BZERO").unwrap_or(Ok(0))?;
            let xtension = header.get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let pix = hdu_list.get_data(&hdu);
            let data = convert_bytes(pix.raw_bytes(), bzero as u16);
            return Ok(
                ArrayD::from_shape_vec(IxDyn(&[naxis2 as usize, naxis1 as usize]), data)
                    .map_err(|_| "Failed to create ArrayD with the given shape")?,
            );
        }
    }
    Err(fitsrs::error::Error::DynamicError(
        "No image data found".to_string(),
    ))
}

#[derive(Serialize, Deserialize)]
pub struct FitsImage<'a> {
    #[serde(borrow)]
    data: &'a [u8],
}

impl<'a> FitsImage<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        FitsImage { data }
    }
    pub fn read_image(&self) -> Result<ArrayD<u16>, fitsrs::error::Error> {
        let reader = Cursor::new(self.data);
        let fits = Fits::from_reader(reader);
        read_fits(fits)
    }
}
