use std::sync::Arc;

use eframe::{egui_glow, glow::HasContext};
use egui::{mutex::Mutex, Pos2};
use egui_glow::glow;
use ndarray::ArrayD;

use crate::analysis::Statistics;

pub struct FitsRender {
    image: FitsImage,
    program: glow::Program,
    vbo: glow::Buffer,
    vao: glow::VertexArray,
    texture: glow::Texture,

    // Image values <= this value are clipped low
    clip_low: f32,

    // Image values >= this value are clipped high
    clip_high: f32,

    histogram_low: f32,
    histogram_mtf: f32,
    histogram_high: f32,

    scale: f32,
    translate: [f32; 2],
}

pub struct FitsImage {
    image: ArrayD<u16>,
    stats: Statistics,
    dirty: bool,
}

impl FitsImage {
    fn new(bytes: ArrayD<u16>) -> FitsImage {
        let stats = Statistics::new(&bytes.view());
        FitsImage {
            image: bytes,
            stats,
            dirty: true,
        }
    }
}

pub struct FitsWidget {
    renderer: Arc<Mutex<FitsRender>>,
}

impl FitsWidget {
    pub fn new<'a>(renderer: Arc<Mutex<FitsRender>>) -> Self {
        Self { renderer }
    }
}

impl egui::Widget for FitsWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Frame::canvas(ui.style())
            .show(ui, |ui| {
                self.custom_painting(ui);
            })
            .response
    }
}

impl FitsWidget {
    fn custom_painting(&self, ui: &mut egui::Ui) {
        let mut renderer = self.renderer.lock();
        let shape = renderer.image.image.shape();

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

            if r.image.dirty {
                r.load_image_data(gl);
                r.image.dirty = false;
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

#[allow(unsafe_code)] // we need unsafe code to use glow
impl FitsRender {
    pub fn new(gl: &glow::Context, image: ArrayD<u16>) -> Option<Self> {
        use glow::HasContext as _;

        let shader_version = egui_glow::ShaderVersion::get(gl);
        let vbo;
        let vao;
        let texture;
        let program;

        unsafe {
            program = gl.create_program().expect("Cannot create program");

            if !shader_version.is_new_shader_interface() {
                tracing::warn!(
                    "Custom 3D painting hasn't been ported to {:?}",
                    shader_version
                );
                return None;
            }

            let vertex_shader_source = include_str!("shaders/fits_vertex.glsl");
            let fragment_shader_source = include_str!("shaders/fits_fragment.glsl");

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let _shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(
                        shader,
                        &format!(
                            "{}\n{}",
                            shader_version.version_declaration(),
                            shader_source
                        ),
                    );
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        panic!(
                            "Failed to compile fits_widget: {}",
                            gl.get_shader_info_log(shader)
                        );
                    }
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            let triangle_vertices: [f32; 12] =
                [0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
            let triangle_vertices_u8: &[u8] = core::slice::from_raw_parts(
                triangle_vertices.as_ptr() as *const u8,
                triangle_vertices.len() * core::mem::size_of::<f32>(),
            );

            vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, triangle_vertices_u8, glow::STATIC_DRAW);

            vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);

            texture = gl.create_texture().expect("Cannot create texture");
        }
        let fits_image = FitsImage::new(image);

        let clip_low = fits_image.stats.clip_low.value as f32 / std::u16::MAX as f32;
        let clip_high = fits_image.stats.clip_high.value as f32 / std::u16::MAX as f32;
        let histogram_high = fits_image.stats.clip_high.value as f32 / std::u16::MAX as f32;
        let histogram_low = fits_image.stats.clip_low.value as f32 / std::u16::MAX as f32;
        let histogram_mtf = (fits_image.stats.median as f32 - 2.8 * fits_image.stats.mad as f32)
            / std::u16::MAX as f32;

        Some(Self {
            image: fits_image,
            program,
            vbo,
            vao,
            texture,
            clip_low,
            clip_high,
            histogram_low,
            histogram_mtf,
            histogram_high,
            scale: 1.0,
            translate: [0.0, 0.0],
        })
    }

    pub fn set_fits(&mut self, data: ArrayD<u16>) {
        println!("set_fits");

        self.image = FitsImage::new(data);

        self.clip_low = self.image.stats.clip_low.value as f32 / std::u16::MAX as f32;
        self.clip_high = self.image.stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.histogram_high = self.image.stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.histogram_low = self.image.stats.clip_low.value as f32 / std::u16::MAX as f32;
        self.histogram_mtf = (self.image.stats.median as f32 - 2.8 * self.image.stats.mad as f32)
            / std::u16::MAX as f32;
    }

    // https://en.wikipedia.org/wiki/Median_absolute_deviation
    // midpoint = median + -2.8*mad (if median < 0.5)

    pub fn load_image_data(&mut self, gl: &glow::Context) {
        println!("load_image_data");

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R16UI as i32,
                self.image.image.shape()[1] as i32,
                self.image.image.shape()[0] as i32,
                0,
                glow::RED_INTEGER,
                glow::UNSIGNED_SHORT,
                Some(std::slice::from_raw_parts(
                    self.image.image.as_ptr() as *const u8,
                    self.image.image.len(),
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
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_texture(self.texture);
        }
    }

    fn paint(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.use_program(Some(self.program));
            gl.active_texture(glow::TEXTURE0);
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "mono_fits").as_ref(),
                0,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "scale").as_ref(),
                self.scale,
            );
            gl.uniform_2_f32_slice(
                gl.get_uniform_location(self.program, "translate").as_ref(),
                &self.translate,
            );

            // Convert image cordinates (0.0-1.0, +y -> down) to opengl coordinates (-1.0, 1.0, +y -> up)
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "M").as_ref(),
                false,
                &[
                    2.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 1.0, 0.0,
                    1.0,
                ],
            );

            // Apply visual zoom and pan.
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "V").as_ref(),
                false,
                &[
                    self.scale,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    self.scale,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    2.0 * self.translate[0],
                    -2.0 * self.translate[1],
                    0.0,
                    1.0,
                ],
            );

            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));

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

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            match gl.get_error() {
                0 => {}
                err => {
                    dbg!(err);
                }
            }
        }
    }
}
