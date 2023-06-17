use std::sync::Arc;

use eframe::{egui_glow, glow::HasContext};
use egui::{mutex::Mutex, Pos2};
use egui_glow::glow;

use super::FitsRender;

pub trait Drawable {
    fn load_data(&mut self, gl: &glow::Context);

    fn draw(&self, gl: &glow::Context, render: &FitsRender);

    unsafe fn destroy(&self, gl: &glow::Context);

    unsafe fn prepare_mesh(&self, gl: &glow::Context, render: &FitsRender) {
        let program = self.get_program();
        let vbo = self.get_vbo();
        let vao = self.get_vao();

        gl.use_program(Some(program));
        gl.active_texture(glow::TEXTURE0);
        gl.uniform_1_i32(gl.get_uniform_location(program, "mono_fits").as_ref(), 0);
        gl.uniform_1_f32(
            gl.get_uniform_location(program, "scale").as_ref(),
            render.scale,
        );
        gl.uniform_2_f32_slice(
            gl.get_uniform_location(program, "translate").as_ref(),
            &render.translate,
        );

        // Convert image cordinates (0.0-1.0, +y -> down) to opengl coordinates (-1.0, 1.0, +y -> up)
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(program, "M").as_ref(),
            false,
            &render.model_transform(),
        );

        // Apply visual zoom and pan.
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(program, "V").as_ref(),
            false,
            &render.view_transform(),
        );

        gl.bind_texture(glow::TEXTURE_2D, Some(render.image_mesh.texture));

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.bind_vertex_array(Some(vao));
    }

    fn get_program(&self) -> glow::Program;
    fn get_vbo(&self) -> glow::Buffer;
    fn get_vao(&self) -> glow::VertexArray;
}

pub struct FitsWidget {
    renderer: Arc<Mutex<FitsRender>>,
}

impl FitsWidget {
    pub fn new(renderer: Arc<Mutex<FitsRender>>) -> Self {
        Self { renderer }
    }
}

impl egui::Widget for FitsWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::canvas(ui.style())
            .show(ui, |ui| self.custom_painting(ui))
            .response
    }
}

impl FitsWidget {
    fn custom_painting(&self, ui: &mut egui::Ui) {
        let mut renderer = self.renderer.lock();
        let shape = renderer.image_mesh.image.shape();
        let (image_width, image_height) = (shape[1], shape[0]);
        let image_ratio = image_width as f32 / image_height as f32;

        let space = ui.available_size();
        let space_ratio = space.x / space.y;

        let image_scale = if space_ratio < image_ratio {
            space.x / image_width as f32
        } else {
            space.y / image_height as f32
        };

        let width = image_width as f32 * image_scale;
        let height = image_height as f32 * image_scale;

        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(width as f32, height as f32),
            egui::Sense::click_and_drag(),
        );

        if let Some(pos) = response.hover_pos() {
            // Calculate pointer's position in frame coordinates (0.0 to 1.0)
            let pos = pos - rect.left_top();
            let frame_pos = Pos2 {
                x: pos.x / width,
                y: pos.y / height,
            };

            // Calculate pointer's position in image coordinates (0.0 to 1.0) from screen coordinates
            let image_pos = Pos2 {
                x: (frame_pos.x - 0.5 - renderer.translate[0]) / renderer.scale + 0.5,
                y: (frame_pos.y - 0.5 - renderer.translate[1]) / renderer.scale + 0.5,
            };
            // Zoom in/out by `zoom_delta`
            renderer.scale *= ui.input().zoom_delta();
            renderer.scale = renderer.scale.max(1.0);

            // Reposition image so pointer is on the same place in on the image.
            renderer.translate[0] = (0.5 - image_pos.x) * renderer.scale + frame_pos.x - 0.5;
            renderer.translate[1] = (0.5 - image_pos.y) * renderer.scale + frame_pos.y - 0.5;

            // Read the pixel value under the mouse cursor.
            let col = (image_pos.x * image_width as f32) as usize;
            let row = (image_pos.y * image_height as f32) as usize;
            let index = [
                row.clamp(0, image_height - 1),
                col.clamp(0, image_width - 1),
            ];
            let _pixel_value =
                Some(renderer.image_mesh.image.get(index).unwrap()).map(|x| x.to_owned());
        }

        // Translate / pan image by dragged amount
        renderer.translate[0] += response.drag_delta().x / width;
        renderer.translate[1] += response.drag_delta().y / height;

        // Limit translate / pan to edge of frame
        let min_t = -0.5 * renderer.scale + 0.5;
        let max_t = 0.5 * renderer.scale - 0.5;

        renderer.translate[0] = renderer.translate[0].clamp(min_t, max_t);
        renderer.translate[1] = renderer.translate[1].clamp(min_t, max_t);

        let renderer = self.renderer.clone();
        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
            let mut r = renderer.lock();
            let gl = painter.gl();

            if r.image_mesh.dirty {
                r.image_mesh.load_data(gl);
                r.image_mesh.dirty = false;
            }
            if r.circles_mesh.dirty {
                r.circles_mesh.load_data(gl);
                r.circles_mesh.dirty = false;
            }
            r.paint(gl);
        });

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        };
        ui.painter().add(callback);
    }
}
