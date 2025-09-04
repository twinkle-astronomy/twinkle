use dioxus::prelude::*;
use ndarray::ArrayD;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlProgram, WebGlShader, WebGlTexture, WebGlBuffer};
use wasm_bindgen::JsCast;

#[derive(Props, Clone, PartialEq)]
pub struct WebGlImageProps {
    pub image_data: ArrayD<u16>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub class: Option<String>,
}

#[component]
pub fn WebGlImage(props: WebGlImageProps) -> Element {
    let mut canvas_ref = use_signal(|| None::<HtmlCanvasElement>);
    let renderer = use_signal(|| None::<WebGlRenderer>);

    let display_width = props.width.unwrap_or(400);
    let display_height = props.height.unwrap_or(300);
    let class_str = props.class.as_deref().unwrap_or("max-w-full h-auto");
    let mut has_webgl_error = use_signal(|| false);
    
    // Initialize WebGL renderer when canvas is ready
    use_effect({
        let image_data = props.image_data.clone();
        let canvas_ref = canvas_ref.clone();
        let mut renderer = renderer.clone();
        let mut has_webgl_error = has_webgl_error.clone();
        
        move || {
            if let Some(canvas) = canvas_ref.read().clone() {
                let image_data = image_data.clone();
                
                web_sys::console::log_1(&"Attempting WebGL initialization...".into());
                match WebGlRenderer::new(canvas, &image_data) {
                    Ok(webgl_renderer) => {
                        web_sys::console::log_1(&"WebGL initialized successfully!".into());
                        renderer.set(Some(webgl_renderer));
                        
                        // Render once
                        if let Some(renderer_instance) = renderer.read().as_ref() {
                            if let Err(e) = renderer_instance.render() {
                                web_sys::console::error_1(&format!("WebGL render error: {:?}", e).into());
                                has_webgl_error.set(true);
                            } else {
                                web_sys::console::log_1(&"WebGL render successful!".into());
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("WebGL initialization failed: {:?}", e).into());
                        has_webgl_error.set(true);
                    }
                }
            }
        }
    });

    let dimensions = get_image_dimensions(&props.image_data);

    rsx! {
        div {
            class: "flex flex-col items-center space-y-2",
            
            if has_webgl_error() {
                div {
                    class: "bg-yellow-100 dark:bg-yellow-900 border border-yellow-400 dark:border-yellow-600 text-yellow-800 dark:text-yellow-200 px-4 py-3 rounded-lg {class_str}",
                    style: "width: {display_width}px; height: {display_height}px;",
                    div {
                        class: "flex flex-col items-center justify-center h-full",
                        p { "⚠️ WebGL Not Supported" }
                        p { class: "text-sm mt-2", "WebGL is required for hardware acceleration" }
                        p { class: "text-xs mt-1", "Image: {dimensions} 16-bit greyscale" }
                    }
                }
            } else {
                canvas {
                    class: "{class_str} border border-gray-300 rounded-lg shadow-sm",
                    width: "{display_width}",
                    height: "{display_height}",
                    onmounted: move |event| {
                        if let Some(element) = event.data().downcast::<web_sys::Element>() {
                            if let Ok(canvas_element) = (*element).clone().dyn_into::<HtmlCanvasElement>() {
                                canvas_ref.set(Some(canvas_element));
                            } else {
                                web_sys::console::error_1(&"Failed to convert element to canvas".into());
                                has_webgl_error.set(true);
                            }
                        } else {
                            web_sys::console::error_1(&"Failed to get web element".into());
                            has_webgl_error.set(true);
                        }
                    }
                }
            }
            
            div {
                class: "text-sm text-gray-600 dark:text-gray-400 text-center",
                if has_webgl_error() {
                    p { "WebGL fallback - Hardware acceleration unavailable" }
                } else {
                    p { "16-bit Greyscale ({dimensions}) - WebGL Accelerated" }
                }
            }
        }
    }
}

pub struct WebGlRenderer {
    gl: GL,
    program: WebGlProgram,
    texture: WebGlTexture,
    vertex_buffer: WebGlBuffer,
}

impl WebGlRenderer {
    pub fn new(canvas: HtmlCanvasElement, image_data: &ArrayD<u16>) -> Result<Self, String> {
        // Get WebGL context
        let gl = canvas
            .get_context("webgl2")
            .map_err(|e| format!("Failed to get WebGL2 context: {:?}", e))?
            .ok_or("WebGL2 context not available".to_string())?
            .dyn_into::<GL>()
            .map_err(|e| format!("Failed to cast to WebGL2 context: {:?}", e))?;

        // Vertex shader source for full-screen quad
        let vertex_shader_source = r#"#version 300 es
            in vec2 position;
            in vec2 texCoord;
            out vec2 vTexCoord;
            
            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                vTexCoord = texCoord;
            }
        "#;

        // Fixed fragment shader for texture rendering
        let fragment_shader_source = r#"#version 300 es
            precision highp float;
            
            in vec2 vTexCoord;
            out vec4 fragColor;
            
            uniform sampler2D uTexture;
            
            void main() {
                // Sample the texture
                float value = texture(uTexture, vTexCoord).r;
                
                // Simple visualization without gamma correction for now
                fragColor = vec4(value, value, value, 1.0);
            }
        "#;

        // Create and compile shaders
        web_sys::console::log_1(&"Compiling vertex shader...".into());
        let vertex_shader = compile_shader(&gl, GL::VERTEX_SHADER, vertex_shader_source)?;
        web_sys::console::log_1(&"Compiling fragment shader...".into());
        let fragment_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, fragment_shader_source)?;
        web_sys::console::log_1(&"Shaders compiled successfully".into());

        // Create program and link shaders
        let program = gl.create_program().ok_or("Failed to create shader program".to_string())?;
        gl.attach_shader(&program, &vertex_shader);
        gl.attach_shader(&program, &fragment_shader);
        gl.link_program(&program);

        if !gl.get_program_parameter(&program, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            let info = gl.get_program_info_log(&program).unwrap_or_default();
            return Err(format!("Failed to link shader program: {}", info));
        }

        gl.use_program(Some(&program));

        // Create vertex buffer for full-screen quad
        let vertices: [f32; 24] = [
            // Position  TexCoord
            -1.0, -1.0,  0.0, 1.0, // Bottom-left
             1.0, -1.0,  1.0, 1.0, // Bottom-right
            -1.0,  1.0,  0.0, 0.0, // Top-left
             1.0,  1.0,  1.0, 0.0, // Top-right
            -1.0,  1.0,  0.0, 0.0, // Top-left (second triangle)
             1.0, -1.0,  1.0, 1.0, // Bottom-right (second triangle)
        ];

        let vertex_buffer = gl.create_buffer().ok_or("Failed to create vertex buffer".to_string())?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));

        unsafe {
            let vertices_array = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vertices_array, GL::STATIC_DRAW);
        }

        // Set up vertex attributes
        let position_attr = gl.get_attrib_location(&program, "position") as u32;
        let tex_coord_attr = gl.get_attrib_location(&program, "texCoord") as u32;

        gl.enable_vertex_attrib_array(position_attr);
        gl.vertex_attrib_pointer_with_i32(position_attr, 2, GL::FLOAT, false, 16, 0);

        gl.enable_vertex_attrib_array(tex_coord_attr);
        gl.vertex_attrib_pointer_with_i32(tex_coord_attr, 2, GL::FLOAT, false, 16, 8);

        // Create and upload texture
        let texture = gl.create_texture().ok_or("Failed to create texture".to_string())?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&texture));

        let image_shape = image_data.shape();
        let image_width = image_shape[1] as i32;
        let image_height = image_shape[0] as i32;

        // Convert 16-bit data to 32-bit float for WebGL
        let float_data: Vec<f32> = image_data.iter()
            .map(|&val| val as f32 / 65535.0)
            .collect();
            
        web_sys::console::log_1(&format!("Texture size: {}x{}, data points: {}", image_width, image_height, float_data.len()).into());

        // Use simpler RGB format instead of R32F
        let rgb_data: Vec<u8> = float_data.iter()
            .flat_map(|&val| {
                let byte_val = (val * 255.0) as u8;
                [byte_val, byte_val, byte_val] // Convert to RGB
            })
            .collect();

        unsafe {
            let rgb_array = js_sys::Uint8Array::view(&rgb_data);
            gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                GL::TEXTURE_2D,
                0,
                GL::RGB as i32,
                image_width,
                image_height,
                0,
                GL::RGB,
                GL::UNSIGNED_BYTE,
                Some(&rgb_array),
            ).map_err(|e| format!("Failed to upload texture data: {:?}", e))?;
        }

        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        // Set texture uniform
        let texture_uniform = gl.get_uniform_location(&program, "uTexture");
        gl.uniform1i(texture_uniform.as_ref(), 0);

        Ok(Self {
            gl,
            program,
            texture,
            vertex_buffer,
        })
    }

    pub fn render(&self) -> Result<(), String> {
        let gl = &self.gl;

        // Set viewport to canvas size  
        let canvas = gl.canvas().unwrap().dyn_into::<HtmlCanvasElement>().unwrap();
        let width = canvas.client_width() as i32;
        let height = canvas.client_height() as i32;
        web_sys::console::log_1(&format!("Canvas size: {}x{}", width, height).into());
        gl.viewport(0, 0, width, height);

        // Clear with a visible color to test if clearing works
        gl.clear_color(0.2, 0.3, 0.8, 1.0); // Blue background to see if clearing works
        gl.clear(GL::COLOR_BUFFER_BIT);
        
        // Check error after clear
        let mut error = gl.get_error();
        if error != GL::NO_ERROR {
            return Err(format!("WebGL error after clear: {}", error));
        }

        gl.use_program(Some(&self.program));
        error = gl.get_error();
        if error != GL::NO_ERROR {
            return Err(format!("WebGL error after use_program: {}", error));
        }
        
        // Re-bind vertex attributes
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.vertex_buffer));
        
        let position_attr = gl.get_attrib_location(&self.program, "position") as u32;
        let tex_coord_attr = gl.get_attrib_location(&self.program, "texCoord") as u32;
        web_sys::console::log_1(&format!("Attribute locations - position: {}, texCoord: {}", position_attr, tex_coord_attr).into());

        gl.enable_vertex_attrib_array(position_attr);
        gl.vertex_attrib_pointer_with_i32(position_attr, 2, GL::FLOAT, false, 16, 0);

        gl.enable_vertex_attrib_array(tex_coord_attr);
        gl.vertex_attrib_pointer_with_i32(tex_coord_attr, 2, GL::FLOAT, false, 16, 8);
        
        error = gl.get_error();
        if error != GL::NO_ERROR {
            return Err(format!("WebGL error after vertex setup: {}", error));
        }
        
        // Bind texture
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.texture));
        
        error = gl.get_error();
        if error != GL::NO_ERROR {
            return Err(format!("WebGL error after texture bind: {}", error));
        }

        // Try to draw
        web_sys::console::log_1(&"About to draw triangles...".into());
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        
        // Check for GL errors
        error = gl.get_error();
        if error != GL::NO_ERROR {
            return Err(format!("WebGL error during draw: {}", error));
        }
        
        web_sys::console::log_1(&"Draw completed successfully".into());

        Ok(())
    }
}

fn compile_shader(
    gl: &GL,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Failed to create shader".to_string())?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if !gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
        let info = gl.get_shader_info_log(&shader).unwrap_or_default();
        return Err(format!("Failed to compile shader: {}", info));
    }

    Ok(shader)
}

fn start_render_loop(renderer: Signal<Option<WebGlRenderer>>, mut has_webgl_error: Signal<bool>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    
    fn request_animation_frame(f: &Closure<dyn FnMut()>) {
        web_sys::window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .unwrap();
    }

    let f = std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        if let Some(renderer_instance) = renderer.read().as_ref() {
            if let Err(e) = renderer_instance.render() {
                web_sys::console::error_1(&format!("WebGL render error: {:?}", e).into());
                has_webgl_error.set(true);
                return;
            }
        }

        // Schedule next frame
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn get_image_dimensions(array: &ArrayD<u16>) -> String {
    if array.ndim() >= 2 {
        let shape = array.shape();
        format!("{}×{}", shape[1], shape[0]) // width × height
    } else {
        "Invalid dimensions".to_string()
    }
}