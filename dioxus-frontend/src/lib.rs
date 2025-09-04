use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use ndarray::ArrayD;

pub mod image_display;
use image_display::ImageDisplay;

pub mod fits;

#[wasm_bindgen(start)]
pub fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut count = use_signal(|| 0);
    let mut show_image = use_signal(|| false);
    
    // Load FITS sample image (cached)
    let sample_image = use_resource(|| async {
        load_sample_fits_image_optimized()
    });

    rsx! {
        div {
            class: "min-h-screen bg-gray-100 dark:bg-gray-900 p-8",
            
            div {
                class: "max-w-6xl mx-auto space-y-8",
                
                // Counter Section
                div {
                    class: "bg-white dark:bg-gray-800 rounded-lg shadow-lg p-8",
                    h1 { 
                        class: "text-4xl font-bold text-center text-gray-800 dark:text-white mb-2",
                        "Hello from Dioxus!" 
                    }
                    h2 { 
                        class: "text-2xl font-semibold text-center text-gray-600 dark:text-gray-300 mb-8",
                        "Counter: {count}" 
                    }
                    
                    div {
                        class: "flex flex-col sm:flex-row gap-4 justify-center mb-8",
                        button {
                            class: "bg-blue-500 hover:bg-blue-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                            onclick: move |_| count += 1,
                            "Increment"
                        }
                        button {
                            class: "bg-red-500 hover:bg-red-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                            onclick: move |_| count -= 1,
                            "Decrement"
                        }
                        button {
                            class: "bg-gray-500 hover:bg-gray-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                            onclick: move |_| count.set(0),
                            "Reset"
                        }
                    }
                }
                
                // Image Display Section
                div {
                    class: "bg-white dark:bg-gray-800 rounded-lg shadow-lg p-8",
                    h2 { 
                        class: "text-2xl font-bold text-center text-gray-800 dark:text-white mb-6",
                        "16-bit Greyscale Image Display" 
                    }
                    
                    div {
                        class: "flex flex-col items-center space-y-4",
                        button {
                            class: "bg-green-500 hover:bg-green-600 text-white font-semibold py-2 px-6 rounded-lg transition-colors duration-200 cursor-pointer",
                            onclick: move |_| show_image.set(!show_image()),
                            if show_image() { "Hide Sample Image" } else { "Show Sample Image" }
                        }
                        
                        if show_image() {
                            div {
                                class: "mt-6",
                                match sample_image.read().as_ref() {
                                    Some(Ok(image_data)) => rsx! {
                                        ImageDisplay {
                                            image_data: image_data.clone(),
                                            width: 400,
                                            height: 300,
                                            class: "rounded-lg shadow-md".to_string(),
                                        }
                                    },
                                    Some(Err(_)) => rsx! {
                                        div {
                                            class: "text-red-500 p-4",
                                            "Failed to load FITS image"
                                        }
                                    },
                                    None => rsx! {
                                        div {
                                            class: "text-gray-500 p-4",
                                            "Loading FITS image..."
                                        }
                                    }
                                }
                            }
                            p {
                                class: "text-center text-gray-600 dark:text-gray-400 text-sm mt-4",
                                "Real 16-bit astronomical image loaded from FITS file (vdB 152)"
                            }
                        }
                    }
                }
                
                p {
                    class: "text-center text-gray-600 dark:text-gray-400 text-sm",
                    "This demonstrates a Dioxus web application with counter and 16-bit image display capabilities."
                }
            }
        }
    }
}

fn load_sample_fits_image_optimized() -> Result<ArrayD<u16>, Box<dyn std::error::Error>> {
    // Embed the FITS file at compile time
    const FITS_DATA: &[u8] = include_bytes!("../vdb_152_Light_Luminance_360_secs_2025-06-27T02-13-46_027.fits");
    
    let full_image = fits::read_fits_from_bytes(FITS_DATA)?;
    
    // Downsample the image for better performance
    // Target maximum size of ~800x600 for display
    let original_shape = full_image.shape();
    let original_height = original_shape[0];
    let original_width = original_shape[1];
    
    // Calculate downsampling factor
    let max_display_size = 800;
    let downsample_factor = (original_width.max(original_height) / max_display_size).max(1);
    
    if downsample_factor > 1 {
        downsample_image(&full_image, downsample_factor)
    } else {
        Ok(full_image)
    }
}

fn downsample_image(image: &ArrayD<u16>, factor: usize) -> Result<ArrayD<u16>, Box<dyn std::error::Error>> {
    let shape = image.shape();
    let height = shape[0];
    let width = shape[1];
    
    let new_height = height / factor;
    let new_width = width / factor;
    
    let mut downsampled_data = Vec::with_capacity(new_height * new_width);
    
    // Simple box filter downsampling
    for y in 0..new_height {
        for x in 0..new_width {
            let mut sum = 0u64;
            let mut count = 0u64;
            
            // Sample from the factor x factor region
            for dy in 0..factor {
                for dx in 0..factor {
                    let orig_y = y * factor + dy;
                    let orig_x = x * factor + dx;
                    
                    if orig_y < height && orig_x < width {
                        sum += image[[orig_y, orig_x]] as u64;
                        count += 1;
                    }
                }
            }
            
            let avg = if count > 0 { (sum / count) as u16 } else { 0 };
            downsampled_data.push(avg);
        }
    }
    
    Ok(ArrayD::from_shape_vec(vec![new_height, new_width], downsampled_data)?)
}

fn create_fallback_image(width: usize, height: usize) -> ArrayD<u16> {
    let mut data = Vec::new();
    
    for y in 0..height {
        for x in 0..width {
            // Create a gradient pattern with some noise
            let gradient_x = (x as f64 / width as f64 * 65535.0) as u16;
            let gradient_y = (y as f64 / height as f64 * 65535.0) as u16;
            
            // Combine gradients and add some pattern
            let value = ((gradient_x as u32 + gradient_y as u32) / 2) as u16;
            
            // Add a circular pattern
            let center_x = width as f64 / 2.0;
            let center_y = height as f64 / 2.0;
            let distance = ((x as f64 - center_x).powi(2) + (y as f64 - center_y).powi(2)).sqrt();
            let max_distance = (center_x.powi(2) + center_y.powi(2)).sqrt();
            let circle_factor = (1.0 - (distance / max_distance).min(1.0)) * 0.3;
            
            let final_value = (value as f64 * (0.7 + circle_factor)) as u16;
            data.push(final_value);
        }
    }
    
    ArrayD::from_shape_vec(vec![height, width], data).unwrap()
}