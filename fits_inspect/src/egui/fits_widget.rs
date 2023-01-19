use std::sync::Arc;

use eframe::{egui_glow, glow::{HasContext}};
use egui::mutex::Mutex;
// use egui::ScrollArea;
use egui_glow::glow;
use fitsio::FitsFile;
use ndarray::{Array2, ArrayD};

use crate::Statistics;
// use image::EncodableLayout;

// use std::cmp;

pub struct FitsWidget {
    /// Behind an `Arc<Mutex<…>>` so we can pass it to [`egui::PaintCallback`] and paint later.
    renderer: Arc<Mutex<FitsRender>>
}

impl FitsWidget {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Self> {
        let gl = cc.gl.as_ref()?;
        Some(Self {
            renderer: Arc::new(Mutex::new(FitsRender::new(gl)?))
        })
    }

    pub fn set_fits(&mut self, gl: &glow::Context, mut fptr: FitsFile) {
        let hdu = fptr.primary_hdu().unwrap();
        let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();
        let stats = Statistics::new(&data.view());
        let mut rt = self.renderer.lock();
        rt.set_image_data(gl, data.into_dimensionality().expect("Bad dimmensions"), stats);
    }
}

impl eframe::App for FitsWidget {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                egui::Frame::canvas(ui.style()).show(ui, |ui| {
                    self.custom_painting(ui);
                });
                
                ui.spacing_mut().slider_width = ui.available_width() - 100.0;
                let mut renderer = self.renderer.lock();
                if ui.add(egui::Slider::new(&mut renderer.histogram_low, 0.0..=1.0).prefix("low: ")).changed() {
                    if renderer.histogram_low > renderer.histogram_high {
                        renderer.histogram_high = renderer.histogram_low;
                    }
                }

                ui.add(egui::Slider::new(&mut renderer.histogram_mtf, 0.0..=1.0).prefix("mid: "));
                if ui.add(egui::Slider::new(&mut renderer.histogram_high, 0.0..=1.0).prefix("high: ")).changed() {
                    if renderer.histogram_low > renderer.histogram_high {
                        renderer.histogram_low = renderer.histogram_high;
                    }
                    
                }

    
                
            });
        });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.renderer.lock().destroy(gl);
        }
    }
}

impl FitsWidget {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (image_width, image_height) = (4144, 2822);
        let image_ratio = image_width as f32 / image_height as f32;

        let space = ui.available_size();
        let space_ratio = space.x / space.y;

        let size_ratio = if space_ratio < image_ratio {
            space.x / image_width as f32
        } else {
            space.y / image_height as f32
        };

        let width = image_width as f32 * size_ratio;
        let height = image_height as f32 * size_ratio;

        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(width as f32, height as f32),
            egui::Sense::click_and_drag(),
        );

        let mut renderer = self.renderer.lock();

        if let Some(_pos) = response.hover_pos() {
            let w = renderer.max_x - renderer.min_x;
            let h = renderer.max_y - renderer.min_y;

            let new_w = w * ui.input().zoom_delta();
            let new_h = h * ui.input().zoom_delta();

            renderer.min_x = renderer.min_x - (w - new_w) / 2.0;
            renderer.min_y = renderer.min_y - (h - new_h) / 2.0;

            renderer.max_x = renderer.max_x + (w - new_w) / 2.0;
            renderer.max_y = renderer.max_y + (w - new_w) / 2.0;
        }

        let drag_scale = response.drag_delta().x / space.x;
        let drag_x = drag_scale * (renderer.max_x - renderer.min_x);
        renderer.min_x -= drag_x;
        renderer.max_x -= drag_x;

        if renderer.min_x < 0.0 && renderer.max_x > 1.0 {
            renderer.min_x = 0.0;
            renderer.max_x = 1.0;
        }

        if renderer.min_x > 1.0 {
            renderer.max_x -= renderer.min_x - 1.0;
            renderer.min_x = 1.0;
        } else if renderer.min_x < 0.0 {
            renderer.max_x -= renderer.min_x;
            renderer.min_x = 0.0;
        }

        if renderer.max_x > 1.0 {
            renderer.min_x -= renderer.max_x - 1.0;
            renderer.max_x = 1.0;
        } else if renderer.max_x < 0.0 {
            renderer.min_x -= renderer.max_x;
            renderer.max_x = 0.0;
        }


        let drag_scale = response.drag_delta().y / space.y;
        let drag_y = drag_scale * (renderer.max_y - renderer.min_y);
        renderer.min_y += drag_y;
        renderer.max_y += drag_y;

        if renderer.min_y < 0.0 && renderer.max_y > 1.0 {
            renderer.min_y = 0.0;
            renderer.max_y = 1.0;
        }

        if renderer.min_y > 1.0 {
            renderer.max_y -= renderer.min_y - 1.0;
            renderer.min_y = 1.0;
        } else if renderer.min_y < 0.0 {
            renderer.max_y -= renderer.min_y;
            renderer.min_y = 0.0;
        }

        if renderer.max_y > 1.0 {
            renderer.min_y -= renderer.max_y - 1.0;
            renderer.max_y = 1.0;
        } else if renderer.max_y < 0.0 {
            renderer.min_y -= renderer.max_y;
            renderer.max_y = 0.0;
        }

        let renderer = self.renderer.clone();

        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
            renderer
                .lock()
                .paint(painter.gl());
        });

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        };
        ui.painter().add(callback);
    }
}

struct FitsRender {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    texture: glow::Texture,

    // Image values <= this value are clipped low
    clip_low: f32,

    // Image values >= this value are clipped high
    clip_high: f32,

    histogram_low: f32,
    histogram_mtf: f32,
    histogram_high: f32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32
}

#[allow(unsafe_code)] // we need unsafe code to use glow
impl FitsRender {
    fn new(gl: &glow::Context) -> Option<Self> {
        use glow::HasContext as _;

        // let shader_version = egui_glow::ShaderVersion::get(gl);

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            // if !shader_version.is_new_shader_interface() {
            //     tracing::warn!(
            //         "Custom 3D painting hasn't been ported to {:?}",
            //         shader_version
            //     );
            //     return None;
            // }

            let (vertex_shader_source, fragment_shader_source) = (
                r#"
                    const vec2 verts[6] = vec2[6](
                        vec2(-1.0, 1.0),
                        vec2(1.0, 1.0),
                        vec2(1.0, -1.0),

                        vec2(-1.0, 1.0),
                        vec2(-1.0, -1.0),
                        vec2(1.0, -1.0)
                    );

                    out vec2 UV;
                    uniform float center_x;
                    uniform float center_y;

                    uniform float min_x;
                    uniform float min_y;

                    uniform float max_x;
                    uniform float max_y;

                    vec2 texture_verts[6] = vec2[6](
                        vec2(min_x, max_y),
                        vec2(max_x, max_y),
                        vec2(max_x, min_y),

                        vec2(min_x, max_y),
                        vec2(min_x, min_y),
                        vec2(max_x, min_y)
                    );

                    void main() {
                        gl_Position = vec4(verts[gl_VertexID], 0.0,  1.0);
                        UV = texture_verts[gl_VertexID];
                    }
                "#,
                r#"
                    precision highp float;
                    in vec2 UV;
                    out vec4 color;
                    uniform float clip_low;
                    uniform float clip_high;

                    uniform float histogram_low;
                    uniform float histogram_high;
                    uniform float histogram_mtf;

                    uniform sampler2D mono_fits;
                    void main() {
                        double x = texture( mono_fits, vec2(UV.x, 1.0-UV.y) ).r;
                        double h_low = histogram_low;
                        double h_high = histogram_high;
                        double h_mtf = histogram_mtf;

                        // 0.5 into [0.25 -> .75] = 0.5
                        // 0.5 - 0.25 = 0.25
                        // 5  = 0.5

                        if (x >= clip_high) {
                                color.r = 0.5f;
                                color.g = 0.25f;
                                color.b = 0.25f;
                        } else if (x <= clip_low) {
                            color.r = 0.25f;
                            color.g = 0.5f;
                            color.b = 0.25f;
                        } else {

                             x = (x - h_low) / (h_high - h_low);
                             x =            ((h_mtf - 1.0)* x) /
                                 ((2*h_mtf - 1.0) * x - h_mtf);

                            color = vec4(x, x, x, 1.0);
                        }
                    }
                "#,
            );

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
                            "#version 400\n",
                            //                            shader_version.version_declaration(),
                            shader_source
                        ),
                    );
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        panic!(
                            "Failed to compile custom_3d_glow: {}",
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

            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vertex_array));


            let texture = gl.create_texture().expect("Cannot create texture");

            Some(Self {
                program,
                vertex_array,
                texture,
                clip_low: 0.0f32,
                clip_high: 1.0f32,
                histogram_low: 0.0,
                histogram_mtf: 0.5,
                histogram_high: 1.0,
                min_x: 0.0, min_y: 0.0,
                max_x: 1.0, max_y: 1.0
            })
        }
    }

    pub fn set_image_data(&mut self,  gl: &glow::Context, data: Array2<u16>, stats: Statistics) {
        self.clip_low = (stats.clip_low.value as f32) / (std::u16::MAX as f32);
        self.clip_high = (stats.clip_high.value as f32) / (std::u16::MAX as f32);

        unsafe {
            self.texture = gl.create_texture().expect("Cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RED as i32,
                data.shape()[1] as i32,
                data.shape()[0] as i32,
                0,
                glow::RED,
                glow::UNSIGNED_SHORT,
                Some(std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len())),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
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

            // gl.generate_mipmap(glow::TEXTURE_2D);
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vertex_array);
        }
    }

    fn paint(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.use_program(Some(self.program));
            gl.active_texture(glow::TEXTURE0);
            gl.uniform_1_i32(gl.get_uniform_location(self.program, "mono_fits").as_ref(), 0);

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
                gl.get_uniform_location(self.program, "histogram_low").as_ref(),
                self.histogram_low as f32,
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "histogram_high").as_ref(),
                self.histogram_high as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "histogram_mtf").as_ref(),
                self.histogram_mtf as f32,
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "min_x").as_ref(),
                self.min_x as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "min_y").as_ref(),
                self.min_y as f32,
            );

            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "max_x").as_ref(),
                self.max_x as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "max_y").as_ref(),
                self.max_y as f32,
            );

            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            match gl.get_error() {
                0 => {},
                err => {
                    dbg!(err);
                }
            }

        }
    }
}
