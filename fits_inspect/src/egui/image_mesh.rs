use std::sync::Arc;

use eframe::glow::{self, HasContext};
use ndarray::ArrayD;

use super::{fits_widget::Drawable, FitsRender};

pub struct ImageMesh {
    pub texture: glow::Texture,
    pub image: Arc<ArrayD<u16>>,
    pub program: glow::Program,
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

    fn get_program(&self) -> glow::Program {
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

        /******* Image stuff *******/
        unsafe {
            let triangle_vertices =
                FitsRender::image_canvas(self.image.shape()[1], self.image.shape()[0]);
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
                self.image.shape()[1] as i32,
                self.image.shape()[0] as i32,
                0,
                glow::RED_INTEGER,
                glow::UNSIGNED_SHORT,
                Some(std::slice::from_raw_parts(
                    self.image.as_ptr() as *const u8,
                    self.image.len(),
                )),
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
        /**************************/
    }
}
