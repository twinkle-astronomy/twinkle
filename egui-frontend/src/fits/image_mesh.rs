use eframe::glow::{self, HasContext};
use log::error;
use ndarray::{Array, ArrayD, Ix2};

use super::{fits_widget::Drawable, FitsRender};
use wasm_bindgen_futures::wasm_bindgen;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

pub struct ImageMesh {
    pub texture: glow::Texture,
    pub image: ArrayD<u16>,
    pub shape: [usize; 2],
    pub program: <eframe::glow::Context as HasContext>::Program,
    pub vbo: glow::Buffer,
    pub vao: glow::VertexArray,

    // Image values <= this value are clipped low
    pub clip_low: f32,

    // Image values >= this value are clipped high
    pub clip_high: f32,

    pub histogram_low: f32,
    pub histogram_mtf: f32,
    pub histogram_high: f32,

    pub dirty: bool,
}

fn downsample_image(image: &ArrayD<u16>) -> ArrayD<u16> {
    // Convert to 2D view for easier processing
    let image_view = image.view().into_dimensionality::<Ix2>().unwrap();
    let shape = image_view.shape();
    
    // Calculate new dimensions
    let new_height = shape[0] / 2;
    let new_width = shape[1] / 2;
    
    // Create a 2x2 kernel
    let kernel = Ix2(2, 2);
    
    // Use windows_with_stride to iterate over 2x2 non-overlapping blocks
    let window_values: Vec<u16> = image_view
        .windows_with_stride(kernel.clone(), kernel)
        .into_iter()
        .map(|x| {
            (x.iter().map(|&p| p as f32).sum::<f32>() / x.len() as f32).round() as u16
        })
        .collect();
    
    // Reshape the flat vector into a 2D array
    let downsampled = Array::from_shape_vec((new_height, new_width), window_values)
        .expect("Shape mismatch when creating downsampled image");
    
    // Convert back to dynamic array
    downsampled.into_dyn()
}

impl Drawable for ImageMesh {
    fn draw(&self, gl: &glow::Context, render: &FitsRender) {
        unsafe {
            self.prepare_mesh(gl, &render);
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "clip_low").as_ref(),
                self.clip_low,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "clip_high").as_ref(),
                self.clip_high,
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "histogram_low")
                    .as_ref(),
                self.histogram_low as f32,
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "histogram_high")
                    .as_ref(),
                self.histogram_high as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "histogram_mtf")
                    .as_ref(),
                self.histogram_mtf as f32,
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 6);
        }
    }

    fn get_program(&self) -> <eframe::glow::Context as HasContext>::Program {
        self.program
    }

    fn get_vbo(&self) -> glow::Buffer {
        self.vbo
    }

    fn get_vao(&self) -> glow::VertexArray {
        self.vao
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_program(self.get_program());
        gl.delete_vertex_array(self.get_vao());
        gl.delete_buffer(self.get_vbo());
        gl.delete_texture(self.texture);
    }

    fn load_data(&mut self, gl: &glow::Context) {
        // println!("load_image_data");
        let start = now();

        /******* Image stuff *******/
        unsafe {
            let max_texture_size = gl.get_parameter_f32(glow::MAX_TEXTURE_SIZE);
            while self.shape[0] as f32 > max_texture_size || self.shape[1] as f32 > max_texture_size {
                self.image = downsample_image(&self.image);
                self.shape[0] = self.image.shape()[0];
                self.shape[1] = self.image.shape()[1];
            }
            let triangle_vertices =
                FitsRender::image_canvas(self.shape[1], self.shape[0]);
            let triangle_vertices_u8: &[u8] = core::slice::from_raw_parts(
                triangle_vertices.as_ptr() as *const u8,
                triangle_vertices.len() * core::mem::size_of::<f32>(),
            );


            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, triangle_vertices_u8, glow::STATIC_DRAW);

            gl.bind_vertex_array(Some(self.vao));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);

            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R16UI as i32,
                self.shape[1] as i32,
                self.shape[0] as i32,
                0,
                glow::RED_INTEGER,
                glow::UNSIGNED_SHORT,
                eframe::glow::PixelUnpackData::Slice(Some(std::slice::from_raw_parts(
                    self.image.as_ptr() as *const u8,
                    self.image.len()* core::mem::size_of::<u16>(),
                ))),
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_BORDER as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_BORDER as i32,
            );
        }

        error!("load_data: {:?}ms", now() - start);
        /**************************/
    }
}
