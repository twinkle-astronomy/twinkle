use eframe::glow;
use fitsrs::Fits;
use futures::executor::block_on;
use indi::client::active_device::ActiveDevice;
use std::{
    collections::HashMap,
    io::Cursor,
    sync::Arc,
};

use egui::Ui;

use ndarray::{ArrayD, IxDyn};

use crate::fits::{analysis::Statistics, FitsRender, FitsWidget};

use super::tab;

#[derive(Default)]
pub struct Device {
    group: tab::TabView,
    parameters: HashMap<String, indi::Parameter>,
    blobs: HashMap<String, Arc<egui::mutex::Mutex<FitsRender>>>,
}

impl Device {
    pub fn set_blob(&mut self, sbv: &indi::serialization::SetBlobVector, gl: &glow::Context) {
        for blob in &sbv.blobs {
            let reader = Cursor::new(blob.value.0.as_ref());
            let fits = Fits::from_reader(reader);
            let data = read_fits(fits).unwrap();
            let stats = Statistics::new(&data.view());
            let mut render = self
                .blobs
                .entry(format!("{}.{}", sbv.name, blob.name))
                .or_insert_with(|| Arc::new(egui::mutex::Mutex::new(FitsRender::new(gl))))
                .lock();

            render.set_fits(data);
            render.auto_stretch(&stats);
        }
    }

    pub fn on_exit(&mut self, gl: &glow::Context) {
        for blob in self.blobs.values() {
            blob.lock().destroy(gl);
        }
        self.blobs.clear();
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
fn read_fits(mut hdu_list: Fits<Cursor<&[u8]>>) -> Result<ArrayD<u16>, fitsrs::error::Error> {
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

impl Device {
    pub fn show(&mut self, ui: &mut Ui, device: &ActiveDevice) {
        let group = {
            let binding = block_on(device.lock());
            let groups = binding.parameter_groups().iter();
            self.group.show(ui, groups)
        };
        ui.separator();
        if let Some(group) = group {
            ui.vertical(|ui| {
                ui.add(crate::indi::widgets::device::Device::new(
                    device,
                    &mut self.parameters,
                    group,
                ));

                for render in self.blobs.values() {
                    ui.add(FitsWidget::new(render.clone()));
                }
            });
        }
    }
}
