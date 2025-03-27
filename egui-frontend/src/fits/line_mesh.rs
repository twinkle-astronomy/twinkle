use eframe::glow::{self, HasContext};

use crate::App;

use super::{fits_render::Elipse, fits_widget::Drawable, FitsRender};

pub struct LineMesh {
    pub elipses: Vec<Elipse>,
    pub program: <eframe::glow::Context as HasContext>::Program,
    pub vbo: glow::Buffer,
    pub vao: glow::VertexArray,

    pub count: i32,
    pub mode: u32,

    pub dirty: bool,
}
impl Drop for LineMesh {
    fn drop(&mut self) {
        let vao = self.get_vao().clone();
        let vbo = self.get_vbo().clone();
        App::run_next_update(Box::new(move |_ctx, frame| {
            if let Some(gl) = frame.gl() {
                unsafe {
                    gl.delete_vertex_array(vao);
                    gl.delete_buffer(vbo);
                }
            }
        }));
    }
}
impl Drawable for LineMesh {
    fn draw(&self, gl: &glow::Context, render: &FitsRender) {
        unsafe {
            self.prepare_mesh(gl, render);
            gl.line_width(2.0);
            gl.draw_arrays(self.mode, 0, self.count);
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

    fn load_data(&mut self, gl: &glow::Context) {
        let mut vertices = Vec::new();

        let points = 40;
        for star in &self.elipses {
            let delta_theta = 2.0 * std::f64::consts::PI / (points as f64);
            for i in 0..points {
                let theta = delta_theta * (i as f64);
                let x0 = (star.a as f64) * theta.cos();
                let y0 = (star.b as f64) * theta.sin();
                let x = x0 * (star.theta as f64).cos() - y0 * (star.theta as f64).sin();
                let y = y0 * (star.theta as f64).cos() + x0 * (star.theta as f64).sin();

                vertices.push(star.x as f32 + x as f32 + 0.5);
                vertices.push(star.y as f32 + y as f32 + 0.5);

                let theta = delta_theta * ((i + 1) as f64);
                let x0 = (star.a as f64) * theta.cos();
                let y0 = (star.b as f64) * theta.sin();
                let x = x0 * (star.theta as f64).cos() - y0 * (star.theta as f64).sin();
                let y = y0 * (star.theta as f64).cos() + x0 * (star.theta as f64).sin();

                vertices.push(star.x as f32 + x as f32 + 0.5);
                vertices.push(star.y as f32 + y as f32 + 0.5);
            }
        }

        self.count = (vertices.len() / 2) as i32;
        unsafe {
            let vertices_u8: &[u8] = core::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                vertices.len() * core::mem::size_of::<f32>(),
            );

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::STATIC_DRAW);

            gl.bind_vertex_array(Some(self.vao));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            /*************************/
        }
    }
}
