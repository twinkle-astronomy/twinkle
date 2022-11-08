#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() {
    tracing_subscriber::fmt::init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Twinkle",
        native_options,
        Box::new(|cc| Box::new(twinkle::TwinkleApp::new(cc))),
    );
}
