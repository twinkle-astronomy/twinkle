use std::sync::Arc;

use eframe::egui_glow;
use egui::mutex::Mutex;
use egui_glow::glow;
use egui::ScrollArea;

use image::io::Reader as ImageReader;
use image::EncodableLayout;

use std::cmp;

pub struct Custom3d {
    /// Behind an `Arc<Mutex<…>>` so we can pass it to [`egui::PaintCallback`] and paint later.
    rotating_triangle: Arc<Mutex<RotatingTriangle>>,
    zoom: f32,
    center_x: f32,
    center_y: f32
}

impl Custom3d {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Self> {
        let gl = cc.gl.as_ref()?;
        Some(Self {
            rotating_triangle: Arc::new(Mutex::new(RotatingTriangle::new(gl)?)),
            zoom: 1.0,
            center_x: 0.5,
            center_y: 0.5,
        })
    }
}

impl eframe::App for Custom3d {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("The triangle is being painted using ");
                ui.hyperlink_to("glow", "https://github.com/grovesNL/glow");
                ui.label(" (OpenGL).");
            });
            ui.label("It's not a very impressive demo, but it shows you can embed 3D inside of egui.");

            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                self.custom_painting(ui);
            });
            ui.label("Drag to rotate!");
        });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.rotating_triangle.lock().destroy(gl);
        }
    }
}


impl Custom3d {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (image_width, image_height) = (1920, 1254);
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

        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(width as f32, height as f32), egui::Sense::click_and_drag());

        if let Some(pos) = response.hover_pos() {
            self.zoom *= ui.input().zoom_delta();
            if self.zoom < 1.0 {
                self.zoom = 1.0;
            }

        }

        self.center_x -= 4.0 * response.drag_delta().x / image_width as f32 / self.zoom;
        self.center_y += 2.0 * response.drag_delta().y / image_height as f32 / self.zoom;

        if self.center_x > 1.0 {
            self.center_x = 1.0;
        } else if self.center_x < 0.0 { 
            self.center_x = 0.0;
        }
        
        if self.center_y > 1.0 {
            self.center_y = 1.0;
        } else if self.center_y < 0.0 { 
            self.center_y = 0.0;
        }

        // Clone locals so we can move them into the paint callback:
        let zoom = self.zoom;
        let center_x = self.center_x;
        let center_y= self.center_y;
        let rotating_triangle = self.rotating_triangle.clone();

        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
            rotating_triangle.lock().paint(painter.gl(), zoom, center_x, center_y);
        });

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        };
        ui.painter().add(callback);
    }
}

struct RotatingTriangle {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    texture: glow::Texture,
}

#[allow(unsafe_code)] // we need unsafe code to use glow
impl RotatingTriangle {
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
                    const vec2 texture_verts[6] = vec2[6](
                        vec2(0.0, 1.0),
                        vec2(1.0, 1.0),
                        vec2(1.0, 0.0),

                        vec2(0.0, 1.0),
                        vec2(0.0, 0.0),
                        vec2(1.0, 0.0)
                    );

                    out vec2 UV;
                    uniform float zoom;
                    uniform float center_x;
                    uniform float center_y;

                    void main() {
                        vec2 center = vec2(center_x, center_y);
                        gl_Position = vec4(verts[gl_VertexID], 0.0, 1.0);
                        float zoom = zoom;
                        UV = (center - 0.5/zoom ) + texture_verts[gl_VertexID] / zoom;
                    }
                "#,
                r#"
                    precision mediump float;
                    in vec2 UV;
                    out vec4 color;
                    uniform sampler2D asdf;
                    void main() {
                        color = vec4(texture( asdf, vec2(UV.x, 1.0-UV.y) ).rgb, 1.0);
                    }
                "#,
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(
                        shader,
                        &format!(
                            "{}\n{}",
                            "#version 330\n",
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
            gl.use_program(Some(program));

            let image = ImageReader::open("/home/cconstantine/community.png").unwrap().decode();
            let mut img = image.unwrap().as_rgb8().unwrap().to_vec();

            let texture = gl.create_texture().expect("Cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(glow::TEXTURE_2D, 
                0, 
                glow::RGB8 as i32,
                1920 as i32,
                1254 as i32,
                0, glow::RGB,
                glow::UNSIGNED_BYTE,
                Some(&img));

            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR_MIPMAP_LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_BORDER as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_BORDER as i32);
            gl.generate_mipmap(glow::TEXTURE_2D);

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");

            Some(Self {
                program,
                vertex_array,
                texture
            })
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vertex_array);
        }
    }

    fn paint(&self, gl: &glow::Context, zoom: f32, center_x: f32, center_y: f32) {
        use glow::HasContext as _;
        unsafe {
            gl.use_program(Some(self.program));
            // println!("myTextureSampler: {:?}", gl.get_uniform_location(self.program, "asdf"));
            gl.active_texture(glow::TEXTURE0);
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "asdf").as_ref(),
                0);

            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "zoom").as_ref(),
                zoom,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "center_x").as_ref(),
                center_x,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.program, "center_y").as_ref(),
                center_y,
            );
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);
        }
    }
}
