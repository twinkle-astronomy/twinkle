use std::{
    fmt::Display,
    net::ToSocketAddrs,
    ops::Deref,
    sync::Arc,
    time::Duration,
};

use indi::{client::{device::ActiveDevice, notify, Notify}, Parameter};
use tokio::net::TcpStream;
use tokio_stream::wrappers::BroadcastStream;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
mod backend;
pub mod flat;

pub trait Action<T> {
    fn status(&self) -> BroadcastStream<std::sync::Arc<T>>;
}

pub struct Telescope {
    pub config: TelescopeConfig,
    pub client: indi::client::Client,
    pub image_client: indi::client::Client,
    runtime: tokio::runtime::Runtime,
}

impl Telescope {
    pub async fn new(addr: impl tokio::net::ToSocketAddrs + Copy + Display, config: TelescopeConfig) -> Telescope {
        // let c = TcpStream::connect(addr.into());
        let client = indi::client::new(
            TcpStream::connect(addr.clone()).await.expect(format!("Unable to connect to {}", addr).as_str()),
            None,
            None,
        )
        .expect("Connecting to INDI server");

        let image_client = indi::client::new(
            TcpStream::connect(addr.clone()).await.expect(format!("Unable to connect to {}", addr).as_str()),
            None,
            None, // Some(&config.primary_camera.clone()),
                  // Some("CCD1"),
        )
        .expect("Connecting to INDI server");

        Telescope {
            config,
            client,
            image_client,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        }
    }

    pub fn new_sync(addr: impl ToSocketAddrs + Copy + Display, config: TelescopeConfig) -> Telescope {
        // let c = TcpStream::connect(addr.into());
        let c = std::net::TcpStream::connect(addr.clone()).expect(format!("Unable to connect to {}", addr).as_str());
        c.set_nonblocking(true).unwrap();
        let c = tokio::net::TcpStream::from_std(c).unwrap();
        let client = indi::client::new(
            c,
            None,
            None,
        )
        .expect("Connecting to INDI server");

        let c = std::net::TcpStream::connect(addr.clone()).expect(format!("Unable to connect to {}", addr).as_str());
        c.set_nonblocking(true).unwrap();
        let c = tokio::net::TcpStream::from_std(c).unwrap();
        let image_client = indi::client::new(
            c,
            None,
            None, // Some(&config.primary_camera.clone()),
                  // Some("CCD1"),
        )
        .expect("Connecting to INDI server");

        Telescope {
            config,
            client,
            image_client,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap(),
        }
    }



    pub async fn get_primary_camera(&self) -> Result<ActiveDevice, notify::Error<()>> {
        self.client.get_device(&self.config.primary_camera).await
    }

    pub async fn get_primary_camera_ccd(
        &self,
    ) -> Result<Arc<Notify<Parameter>>, indi::client::ChangeError<indi::serialization::Command>>
    {
        let image_camera = self
            .image_client
            .get_device::<indi::serialization::Command>(&self.config.primary_camera)
            .await?;
        image_camera
            .enable_blob(Some("CCD1"), indi::BlobEnable::Only)
            .await?;
        Ok(image_camera.get_parameter("CCD1").await?)
    }

    pub async fn get_filter_wheel(&self) -> Result<ActiveDevice, notify::Error<()>> {
        self.client.get_device(&self.config.filter_wheel).await
    }

    pub async fn get_focuser(&self) -> Result<ActiveDevice, notify::Error<()>> {
        self.client.get_device(&self.config.focuser).await
    }

    pub async fn get_flat_panel(&self) -> Result<ActiveDevice, notify::Error<()>> {
        self.client.get_device(&self.config.flat_panel).await
    }

    pub fn root_path(&self) -> String {
        String::from("./Flat/")
    }
}

impl Deref for Telescope {
    type Target = tokio::runtime::Runtime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

pub struct OpticsConfig {
    pub focal_length: f64,
    pub aperture: f64,
}

pub struct TelescopeConfig {
    pub mount: String,
    pub primary_optics: OpticsConfig,
    pub primary_camera: String,
    pub focuser: String,
    pub filter_wheel: String,
    pub flat_panel: String,
}

pub struct AutoFocusConfig {
    pub exposure: Duration,
    pub filter: String,
    pub step: f64,
    pub start_position: f64,
}
pub struct TwinkleApp {
    // // backend: Backend,

    // address: String,

    // selected_device: Option<String>,
    // selected_group: Option<String>,
    // fits_viewer: Option<FitsWidget>,
}

impl Default for TwinkleApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            // address: "localhost:7624".to_owned(),
            // // backend: Default::default(),

            // selected_device: None,
            // selected_group: None,
            // fits_viewer: None::<T>,
        }
    }
}

impl TwinkleApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut _newed: TwinkleApp = Default::default();
        // newed.fits_viewer = FitsWidget::new(cc);
        _newed
    }
}

impl eframe::App for TwinkleApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        /*let Self {
            address,
            backend,
            selected_device,
            selected_group,
            fits_viewer,
        } = self;

        let client_lock = backend.get_client(); //.lock().unwrap();
        let client = client_lock.lock().unwrap();
        let devices = client.devices;

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Indi: ");
                ui.text_edit_singleline(address);
            });

            match backend.get_status() {
                ConnectionStatus::Disconnected => {
                    if ui.button("Connect").clicked() {
                        if let Err(e) = backend.connect(ctx.clone(), address.to_string()) {
                            event!(Level::ERROR, "Connection error: {:?}", e);
                        }
                    }
                }
                ConnectionStatus::Connecting => {
                    ui.label(format!("Connecting to {}", address));
                }
                ConnectionStatus::Initializing => {
                    ui.label(format!("Initializing connection"));
                }
                ConnectionStatus::Connected => {
                    if ui.button("Disconnect").clicked() {
                        if let Err(e) = backend.disconnect() {
                            event!(Level::ERROR, "Disconnection error: {:?}", e);
                        }
                    }
                }
            }

            ui.separator();

            {
                for (name, device) in devices.as_ref().as_ref() {
                    if ui
                        .selectable_value(&mut Some(name), selected_device.as_ref(), name)
                        .clicked()
                    {
                        *selected_device = Some(name.to_string());
                        if device.parameter_groups().len() > 0 {
                            *selected_group = device.parameter_groups()[0].clone();
                        } else {
                            *selected_group = None;
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    egui::warn_if_debug_build(ui);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(fits_viewer) = fits_viewer {
                fits_viewer.update(ctx, _frame);
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show_viewport(ui, |ui, _viewport| {
                    if let Some(device_name) = selected_device {
                        if let Some(device) = devices.get(device_name) {
                            ui.heading(device_name.clone());
                            ui.separator();
                            ui.horizontal(|ui| {
                                for group in device.parameter_groups() {
                                    if ui
                                        .add(egui::SelectableLabel::new(
                                            group == selected_group,
                                            group.clone().unwrap_or("".to_string()),
                                        ))
                                        .clicked()
                                    {
                                        *selected_group = group.clone();
                                    }
                                }
                            });
                            ui.separator();
                            for (name, param) in device
                                .get_parameters()
                                .iter()
                                .filter(|(_, p)| p.get_group() == selected_group)
                            {
                                ui.label(name);
                                ui.separator();

                                match param {
                                    Parameter::TextVector(tv) => {
                                        egui::Grid::new(format!("{}", name)).num_columns(2).show(
                                            ui,
                                            |ui| {
                                                for (text_name, text_value) in &tv.values {
                                                    ui.label(text_name.clone());
                                                    ui.label(text_value.value.clone());
                                                    ui.end_row();
                                                }
                                            },
                                        );
                                    }
                                    Parameter::NumberVector(nv) => {
                                        egui::Grid::new(format!("{}", name)).num_columns(2).show(
                                            ui,
                                            |ui| {
                                                for (number_name, number_value) in &nv.values {
                                                    ui.label(number_name.clone());
                                                    ui.label(format!("{}", number_value.value));
                                                    ui.end_row();
                                                }
                                            },
                                        );
                                    }
                                    Parameter::SwitchVector(sv) => {
                                        ui.horizontal(|ui| {
                                            for (button_name, button_value) in &sv.values {
                                                if ui
                                                    .add(egui::SelectableLabel::new(
                                                        button_value.value == indi::SwitchState::On,
                                                        button_name.clone(),
                                                    ))
                                                    .clicked()
                                                {
                                                    backend.write(
                                                        &indi::Command::NewSwitchVector(
                                                            indi::NewSwitchVector {
                                                                device: device_name.to_string(),
                                                                name: name.to_string(),
                                                                timestamp: None,
                                                                switches: vec![indi::OneSwitch {
                                                                    name: button_name.to_string(),
                                                                    value: indi::SwitchState::On,
                                                                }],
                                                            },
                                                        ),
                                                    ).unwrap_or_else(|e| {dbg!(e);});
                                                }
                                            }
                                        });
                                    }
                                    Parameter::BlobVector(bv) => {
                                        for (name, _blob) in &bv.values {
                                            ui.label(format!("BLOB {}", name));
                                            if ui.button("Images").clicked() {
                                                let enable_blob = indi::EnableBlob {
                                                    device: device_name.clone(),
                                                    name: None,
                                                    enabled: indi::BlobEnable::Also,
                                                };
                                                backend.write(&indi::Command::EnableBlob(
                                                    enable_blob,
                                                )).unwrap_or_else(|e| {dbg!(e);});
                                            }
                                        }
                                    }
                                    _ => {}
                                }

                                ui.end_row();
                            }
                        }
                    }
                });
        });
        // self.backend.tick();
        */
    }
}
