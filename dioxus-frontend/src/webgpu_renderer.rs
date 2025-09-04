use dioxus::prelude::*;
use ndarray::ArrayD;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;
use wasm_bindgen::JsCast;

#[derive(Props, Clone, PartialEq)]
pub struct WebGpuImageProps {
    pub image_data: ArrayD<u16>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub class: Option<String>,
}

#[component]
pub fn WebGpuImage(props: WebGpuImageProps) -> Element {
    let mut canvas_ref = use_signal(|| None::<HtmlCanvasElement>);
    let renderer = use_signal(|| None::<WebGpuRenderer>);

    let display_width = props.width.unwrap_or(400);
    let display_height = props.height.unwrap_or(300);
    let class_str = props.class.as_deref().unwrap_or("max-w-full h-auto");
    let mut has_webgpu_error = use_signal(|| false);
    
    // Initialize WebGPU renderer when canvas is ready
    use_effect({
        let image_data = props.image_data.clone();
        let canvas_ref = canvas_ref.clone();
        let mut renderer = renderer.clone();
        let has_webgpu_error = has_webgpu_error.clone();
        
        move || {
            if let Some(canvas) = canvas_ref.read().clone() {
                let image_data = image_data.clone();
                let mut has_webgpu_error = has_webgpu_error.clone();
                
                wasm_bindgen_futures::spawn_local(async move {
                    web_sys::console::log_1(&"Attempting WebGPU initialization...".into());
                    match WebGpuRenderer::new(canvas, &image_data).await {
                        Ok(webgpu_renderer) => {
                            web_sys::console::log_1(&"WebGPU initialized successfully!".into());
                            renderer.set(Some(webgpu_renderer));
                            
                            // Render the image
                            if let Some(renderer_instance) = renderer.read().as_ref() {
                                if let Err(e) = renderer_instance.render().await {
                                    web_sys::console::error_1(&format!("WebGPU render error: {:?}", e).into());
                                    has_webgpu_error.set(true);
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::error_1(&format!("WebGPU initialization failed: {:?}", e).into());
                            has_webgpu_error.set(true);
                        }
                    }
                });
            }
        }
    });

    let dimensions = get_image_dimensions(&props.image_data);

    rsx! {
        div {
            class: "flex flex-col items-center space-y-2",
            
            if has_webgpu_error() {
                div {
                    class: "bg-yellow-100 dark:bg-yellow-900 border border-yellow-400 dark:border-yellow-600 text-yellow-800 dark:text-yellow-200 px-4 py-3 rounded-lg {class_str}",
                    style: "width: {display_width}px; height: {display_height}px;",
                    div {
                        class: "flex flex-col items-center justify-center h-full",
                        p { "⚠️ WebGPU Not Supported" }
                        p { class: "text-sm mt-2", "Please use Chrome/Edge with WebGPU enabled" }
                        p { class: "text-xs mt-1", "Image: {dimensions} 16-bit greyscale" }
                    }
                }
            } else {
                canvas {
                    class: "{class_str} border border-gray-300 rounded-lg shadow-sm",
                    width: "{display_width}",
                    height: "{display_height}",
                    onmounted: move |event| {
                        // Try to get the element as a web_sys element first
                        if let Some(element) = event.data().downcast::<web_sys::Element>() {
                            if let Ok(canvas_element) = (*element).clone().dyn_into::<HtmlCanvasElement>() {
                                canvas_ref.set(Some(canvas_element));
                            } else {
                                web_sys::console::error_1(&"Failed to convert element to canvas".into());
                                has_webgpu_error.set(true);
                            }
                        } else {
                            web_sys::console::error_1(&"Failed to get web element".into());
                            has_webgpu_error.set(true);
                        }
                    }
                }
            }
            
            div {
                class: "text-sm text-gray-600 dark:text-gray-400 text-center",
                if has_webgpu_error() {
                    p { "WebGPU fallback - Enable WebGPU in browser for hardware acceleration" }
                } else {
                    p { "16-bit Greyscale ({dimensions}) - WebGPU Accelerated" }
                }
            }
        }
    }
}

pub struct WebGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
}

impl WebGpuRenderer {
    pub async fn new(canvas: HtmlCanvasElement, image_data: &ArrayD<u16>) -> Result<Self, Box<dyn std::error::Error>> {
        // Get WebGPU instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        // Get canvas dimensions before creating surface
        let canvas_width = canvas.width();
        let canvas_height = canvas.height();
        
        // Create surface from canvas
        let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas))?;

        // Request adapter
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }).await?;

        // Request device and queue
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            }
        ).await?;

        // Configure surface        
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: canvas_width,
            height: canvas_height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        
        surface.configure(&device, &surface_config);

        // Create shaders
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image.wgsl").into()),
        });

        // Create texture for 16-bit image data
        let image_shape = image_data.shape();
        let image_width = image_shape[1] as u32;
        let image_height = image_shape[0] as u32;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("16-bit Image Texture"),
            size: wgpu::Extent3d {
                width: image_width,
                height: image_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload 16-bit image data to texture
        let raw_data: Vec<u8> = image_data.iter()
            .flat_map(|&val| val.to_le_bytes())
            .collect();

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &raw_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image_width * 2), // 2 bytes per u16
                rows_per_image: Some(image_height),
            },
            texture.size(),
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout and bind group
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Uint,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("bind_group"),
        });

        // Create render pipeline
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Create vertex buffer for full-screen quad
        let vertices: &[[f32; 4]] = &[
            [-1.0, -1.0, 0.0, 1.0], // Bottom-left
            [ 1.0, -1.0, 1.0, 1.0], // Bottom-right
            [-1.0,  1.0, 0.0, 0.0], // Top-left
            [ 1.0,  1.0, 1.0, 0.0], // Top-right
            [-1.0,  1.0, 0.0, 0.0], // Top-left (second triangle)
            [ 1.0, -1.0, 1.0, 1.0], // Bottom-right (second triangle)
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            render_pipeline,
            texture,
            texture_view,
            sampler,
            bind_group,
            vertex_buffer,
        })
    }

    pub async fn render(&self) -> Result<(), Box<dyn std::error::Error>> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn get_image_dimensions(array: &ArrayD<u16>) -> String {
    if array.ndim() >= 2 {
        let shape = array.shape();
        format!("{}×{}", shape[1], shape[0]) // width × height
    } else {
        "Invalid dimensions".to_string()
    }
}