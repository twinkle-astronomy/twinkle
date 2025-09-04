use dioxus::prelude::*;
use ndarray::ArrayD;

#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d, ImageData};

#[derive(Props, Clone, PartialEq)]
pub struct ImageDisplayProps {
    pub image_data: ArrayD<u16>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub class: Option<String>,
}

#[component]
pub fn ImageDisplay(props: ImageDisplayProps) -> Element {
    // Use use_memo for data URL generation with caching
    let image_data_url = use_memo({
        let image_data = props.image_data.clone();
        move || convert_16bit_to_data_url(&image_data)
    });

    let display_width = props.width.unwrap_or(400);
    let display_height = props.height.unwrap_or(300);
    
    let class_str = props.class.as_deref().unwrap_or("max-w-full h-auto");
    let dimensions = get_image_dimensions(&props.image_data);

    rsx! {
        div {
            class: "flex flex-col items-center space-y-2",
            
            if let Some(data_url) = image_data_url() {
                img {
                    src: "{data_url}",
                    alt: "16-bit Greyscale Image",
                    width: "{display_width}",
                    height: "{display_height}",
                    class: "{class_str} border border-gray-300 rounded-lg shadow-sm",
                }
            } else {
                div {
                    class: "flex items-center justify-center bg-gray-200 dark:bg-gray-700 rounded-lg shadow-sm",
                    style: "width: {display_width}px; height: {display_height}px;",
                    p {
                        class: "text-gray-500 dark:text-gray-400",
                        "Failed to render image"
                    }
                }
            }
            
            div {
                class: "text-sm text-gray-600 dark:text-gray-400 text-center",
                p { "16-bit Greyscale ({dimensions})" }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn convert_16bit_to_data_url(array: &ArrayD<u16>) -> Option<String> {
    use wasm_bindgen::JsCast;
    
    // Ensure we have a 2D array
    if array.ndim() != 2 {
        return None;
    }
    
    let shape = array.shape();
    let height = shape[0] as u32;
    let width = shape[1] as u32;
    
    // Convert 16-bit values to 8-bit for display
    let min_val = *array.iter().min()?;
    let max_val = *array.iter().max()?;
    
    let range = if max_val > min_val { max_val - min_val } else { 1 };
    
    // Create RGBA data (grayscale with alpha)
    let mut rgba_data = Vec::with_capacity((height * width * 4) as usize);
    
    for row in 0..height {
        for col in 0..width {
            let value = array[[row as usize, col as usize]];
            // Scale from 16-bit range to 8-bit
            let scaled = ((value - min_val) as f64 / range as f64 * 255.0) as u8;
            
            // Add RGBA values (grayscale with full alpha)
            rgba_data.push(scaled); // R
            rgba_data.push(scaled); // G
            rgba_data.push(scaled); // B
            rgba_data.push(255);    // A
        }
    }
    
    // Use Canvas to create the image
    let window = web_sys::window()?;
    let document = window.document()?;
    let canvas = document.create_element("canvas").ok()?;
    let canvas: HtmlCanvasElement = canvas.dyn_into().ok()?;
    
    canvas.set_width(width);
    canvas.set_height(height);
    
    let context = canvas
        .get_context("2d").ok()??
        .dyn_into::<CanvasRenderingContext2d>().ok()?;
    
    // Create ImageData from our RGBA array
    let image_data = ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&rgba_data),
        width,
        height,
    ).ok()?;
    
    // Put the image data on the canvas
    context.put_image_data(&image_data, 0.0, 0.0).ok()?;
    
    // Convert canvas to data URL
    canvas.to_data_url().ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn convert_16bit_to_data_url(_array: &ArrayD<u16>) -> Option<String> {
    // Fallback for non-WASM targets
    // This won't actually work but prevents compilation errors
    None
}

fn get_image_dimensions(array: &ArrayD<u16>) -> String {
    if array.ndim() >= 2 {
        let shape = array.shape();
        format!("{}×{}", shape[1], shape[0]) // width × height
    } else {
        "Invalid dimensions".to_string()
    }
}