use std::sync::OnceLock;

use eframe::{
    egui_glow,
    glow::{Context, HasContext},
};
use egui_glow::glow;
use ndarray::{ArrayD, IxDyn};

use fits_widget::Drawable;
use twinkle_api::analysis::Statistics;

use super::{fits_widget, image_mesh::ImageMesh, line_mesh::LineMesh};

static IMAGE_SHADER_PROGRAM: OnceLock<<eframe::glow::Context as HasContext>::Program> =
    OnceLock::new();
static CIRCLE_SHADER_PROGRAM: OnceLock<<eframe::glow::Context as HasContext>::Program> =
    OnceLock::new();

#[derive(Debug)]
pub struct Elipse {
    pub x: f32,
    pub y: f32,

    pub a: f32,
    pub b: f32,

    pub theta: f32,
}

#[derive(Clone, Debug)]
pub struct Circle {
    pub x: f32,
    pub y: f32,

    pub r: f32,
}

impl From<Circle> for Elipse {
    fn from(value: Circle) -> Self {
        Elipse {
            x: value.x as f32,
            y: value.y as f32,

            a: value.r,
            b: value.r,

            theta: 0.0,
        }
    }
}

pub struct FitsRender {
    pub image_mesh: ImageMesh,
    pub circles_mesh: LineMesh,

    pub scale: f32,
    pub translate: [f32; 2],
}

#[allow(unsafe_code)] // we need unsafe code to use glow
impl FitsRender {
    fn get_image_program(gl: &Context) -> <eframe::glow::Context as HasContext>::Program {
        IMAGE_SHADER_PROGRAM
            .get_or_init(|| unsafe {
                Self::create_program(
                    gl,
                    include_str!("shaders/fits_vertex.glsl"),
                    include_str!("shaders/fits_fragment.glsl"),
                )
            })
            .clone()
    }
    fn get_circle_program(gl: &Context) -> <eframe::glow::Context as HasContext>::Program {
        CIRCLE_SHADER_PROGRAM
            .get_or_init(|| unsafe {
                Self::create_program(
                    gl,
                    include_str!("shaders/fits_vertex.glsl"),
                    include_str!("shaders/circle_fragment.glsl"),
                )
            })
            .clone()
    }
    unsafe fn create_program(
        gl: &Context,
        vertex_shader_source: &str,
        fragment_shader_source: &str,
    ) -> <eframe::glow::Context as HasContext>::Program {
        let shader_version = egui_glow::ShaderVersion::get(gl);
        let program = gl.create_program().expect("Cannot create program");

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
        program
    }

    #[rustfmt::skip]
    pub fn image_canvas(width: usize, height: usize) -> [f32; 12] {
        [
            0.0,          height as f32,
            width as f32, height as f32,
            width as f32, 0.0,
            0.0,          height as f32,
            0.0,          0.0,
            width as f32, 0.0
        ]
    }

    pub fn new(gl: &glow::Context) -> Self {
        let shader_version = egui_glow::ShaderVersion::get(gl);
        let texture;

        let image_mesh;
        let circles_mesh;

        unsafe {
            if !shader_version.is_new_shader_interface() {
                tracing::error!(
                    "Custom 3D painting hasn't been ported to {:?}",
                    shader_version
                );
            }

            let program = Self::get_image_program(gl);
            let vbo = gl.create_buffer().unwrap();
            let vao = gl.create_vertex_array().unwrap();

            let clip_low = 0.0;
            let clip_high = std::u16::MAX as f32;
            let histogram_high = 1.0;
            let histogram_low = 0.0;
            let histogram_mtf = 0.5;

            texture = gl.create_texture().expect("Cannot create texture");
            let shape = [10, 10];

            // Create the two PBO buffers
            let pbo1 = gl.create_buffer().expect("Cannot create PBO buffer 1");
            let pbo2 = gl.create_buffer().expect("Cannot create PBO buffer 2");

            let image = ArrayD::<u16>::ones(IxDyn(&shape)); //ArrayD::<u16>::zeros(IxDyn(&shape));
            image_mesh = ImageMesh {
                texture,
                image,
                shape,
                program,
                vbo,
                vao,
                clip_low,
                clip_high,
                histogram_low,
                histogram_mtf,
                histogram_high,
                dirty: true,

                // Initialize the new PBO-related fields
                pbo_buffers: [pbo1, pbo2],
                current_pbo: 0,
                pbo_initialized: false, // Will be initialized in first load_data call
            };

            let program = Self::get_circle_program(gl);
            let vbo = gl.create_buffer().unwrap();
            let vao = gl.create_vertex_array().unwrap();
            circles_mesh = LineMesh {
                elipses: vec![],
                program,
                vbo,
                vao,
                count: 0,
                mode: glow::LINES,
                dirty: true,
            };
        }

        Self {
            image_mesh,
            circles_mesh,
            scale: 1.0,
            translate: [0.0, 0.0],
        }
    }

    pub fn set_fits(&mut self, data: ArrayD<u16>) {
        if data != self.image_mesh.image {
            self.image_mesh.shape[0] = data.shape()[0];
            self.image_mesh.shape[1] = data.shape()[1];
            self.image_mesh.image = data;
            self.image_mesh.dirty = true;
        }
    }

    pub fn set_elipses(&mut self, stars: impl IntoIterator<Item = impl Into<Elipse>>) {
        self.circles_mesh.elipses = stars.into_iter().map(|x| x.into()).collect();
        self.circles_mesh.dirty = true;
    }

    pub fn auto_stretch(&mut self, stats: &Statistics) {
        self.image_mesh.clip_low = stats.clip_low.value as f32 / std::u16::MAX as f32;
        self.image_mesh.clip_high = stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.image_mesh.histogram_high = stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.image_mesh.histogram_low =
            (stats.median as f32 + -2.8 * stats.mad as f32).max(0.0) / u16::MAX as f32;
        self.image_mesh.histogram_mtf =
            (stats.median as f32 - 2.8 * stats.mad as f32) / std::u16::MAX as f32;
    }

    pub fn reset_stretch(&mut self, stats: &Statistics) {
        self.image_mesh.clip_low = stats.clip_low.value as f32 / std::u16::MAX as f32;
        self.image_mesh.clip_high = stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.image_mesh.histogram_high = stats.clip_high.value as f32 / std::u16::MAX as f32;
        self.image_mesh.histogram_low = stats.clip_low.value as f32 / std::u16::MAX as f32;
        self.image_mesh.histogram_mtf = 0.5;
    }
    // https://en.wikipedia.org/wiki/Median_absolute_deviation
    // midpoint = median + -2.8*mad (if median < 0.5)

    #[rustfmt::skip]
    pub fn model_transform(&self) -> [f32; 16] {
        let w = self.image_mesh.shape[1] as f32;
        let h = self.image_mesh.shape[0] as f32;
        [
             2.0 / w,  0.0,      0.0,  0.0,
             0.0,     -2.0 / h , 0.0,  0.0,
             0.0,      0.0,      1.0,  0.0,
            -1.0,      1.0,      0.0,  1.0,
        ]
    }

    #[rustfmt::skip]
    pub fn view_transform(&self) -> [f32; 16] {
        let tx = self.translate[0];
        let ty = self.translate[1];
        let scale = self.scale;
        [
            scale,  0.0,    0.0,  0.0,
            0.0,    scale,  0.0,  0.0,
            0.0,    0.0,    1.0,  0.0,
            2.0*tx,-2.0*ty, 0.0,  1.0,
        ]
    }

    pub fn paint(&mut self, gl: &glow::Context) {
        // use glow::HasContext as _;
        unsafe {
            self.image_mesh.draw(gl, self);
            self.circles_mesh.draw(gl, self);

            match gl.get_error() {
                0 => {}
                err => {
                    dbg!(err);
                }
            }
        }
    }
}
