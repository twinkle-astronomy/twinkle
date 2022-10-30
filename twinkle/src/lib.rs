/// We derive Deserialize/Serialize so we can persist app state on shutdown.
use std::sync::{Arc, Mutex};
// use std::sync::mpsc;
// use std::thread;
// use std::thread::JoinHandle;

use tracing::{event, instrument, Level};

use indi::DeError;

use tokio::task;

// #[derive(Debug)]
// struct ConnectionWorker {
//     connection: Arc<Mutex<indi::Connection>>,
//     thread_handle: JoinHandle<Result<(), DeError>>
// }

// impl ConnectionWorker {
//     pub fn new(sender: mpsc::Sender<Option<Result<Command, DeError>>>, address: &str) -> std::io::Result<ConnectionWorker> {
//         let connection = Arc::new(Mutex::new(indi::Connection::new(address)?));
//         let thread_connection = Arc::clone(&connection);

//         let thread_handle = thread::spawn(move || -> Result<(), indi::DeError> {
//             event!(Level::TRACE, "Starting connection Thread");


//             let iter = {
//                 let connection = thread_connection.lock().expect("Mutex");
//                 connection.command_iter()?
//             };

//             for command in iter {
//                 sender.send(Some(command)).unwrap();
//             }
//             sender.send(None).unwrap();
//             event!(Level::TRACE, "Finishing connection Thread");
//             Ok(())
//         });

//         Ok(ConnectionWorker {
//             connection,
//             thread_handle
//         })
//     }

//     pub fn send(&self, command: &indi::Command) -> Result<(), DeError>{
//         let mut connection = self.connection.lock().expect("Mutex");
//         connection.send(command)
//     }

//     pub fn stop(self) -> Result<(), DeError> {
//         {
//             let connection = self.connection.lock().expect("Mutex");
//             connection.disconnect()?;
//         }
//         self.thread_handle.join().unwrap()?;
//         Ok(())
//     }
// }
#[derive(Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Initializing,
    Connected,
    
}

struct Disconnector {
    status: Arc<Mutex<ConnectionStatus>>
}

impl Drop for Disconnector {
    fn drop (&mut self) {
       let mut l = self.status.lock().unwrap();
       *l = ConnectionStatus::Disconnected;
        event!(Level::INFO, "Done with connection");
    }
}

struct Backend {
    client: Arc<Mutex<indi::Client>>,
    connection: Arc<Mutex<Option<indi::Connection>>>,
    connection_status: Arc<Mutex<ConnectionStatus>>
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(indi::Client::new())),
            connection: Arc::new(Mutex::new(None)),
            connection_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected))
        }
    }
}

impl Drop for Backend {
    fn drop (&mut self) {
        let mut locked_connection = self.connection.lock().unwrap();
        if let Some(connection) = &mut *locked_connection {
            _ = connection.disconnect();
        }
    }
}
impl Backend {
    #[instrument(skip(self))]
    fn connect(&mut self, address: String) -> Result<(), indi::DeError> {
        let runtime_client = Arc::clone(&self.client);
        let runtime_connection = Arc::clone(&self.connection);
        let runtime_connection_status = Arc::clone(&self.connection_status);
        self.disconnect()?;

        // let address = address.clone();
        task::spawn_blocking( move || -> Result<(), DeError> {
            let _guard = Disconnector{ status: Arc::clone(&runtime_connection_status) };

            let iter = {
                let mut locked_connection = runtime_connection.lock().unwrap();
                if let Some(connection) = &mut *locked_connection {
                    _ = connection.disconnect();
                }

                event!(Level::INFO, "Connecting to {}", address);
                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Connecting;
                }

                let mut connection = indi::Connection::new("localhost:7624")?;
                event!(Level::INFO, "Connected, requesting properties");
                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Connecting;
                }

                connection.send(&indi::GetProperties {
                    version: indi::INDI_PROTOCOL_VERSION.to_string(),
                    device: None,
                    name: None,
                })?;

                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Connected;
                }
                let iter = connection.command_iter()?;
                *locked_connection = Some(connection);                
                iter
            };

            for command in iter {
                // event!(Level::INFO, "Command: {:?}", command);
                match command {
                    Ok(command) => {
                        let mut client = runtime_client.lock().unwrap();
                        if let Err(e) = client.update(command) {
                            println!("error: {:?}", e)
                        }
                    }
                    Err(e) => match e {
                        e => println!("error: {:?}", e),
                    },
                }
            }
            
            Ok(())
        });

        Ok(())
    }

    fn get_status(&self) -> ConnectionStatus{
        let l = self.connection_status.lock().unwrap();
        l.clone()
    }


    #[instrument(skip(self))]
    fn disconnect(&mut self) -> Result<(), DeError> {
        let connection = self.connection.lock();
        match connection {
            Ok(mut connection) => {
                if let Some(connection) = &mut *connection {
                    connection.disconnect()?;
                    
                }   
                *connection = None;             
            }
            Err(e) => {
                event!(Level::ERROR, "Error disconnecting: {:?}", e);
            }
        }

        Ok(())
    }

}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TwinkleApp {

    #[serde(skip)]
    backend: Backend,

    address: String,


    value: f32,
}


impl Default for TwinkleApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            address: "localhost:7624".to_owned(),
            backend: Default::default(),
            value: 2.7,
        }
    }
}

impl TwinkleApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        match cc.storage {
            Some(storage) => eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default(),
            None => Default::default()
        }
    }
}

impl eframe::App for TwinkleApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { address, backend, value } = self;

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Indi: ");
                ui.text_edit_singleline(address);
            });

            match backend.get_status() {
                ConnectionStatus::Disconnected => {
                    if ui.button("Connect").clicked() {
                        if let Err(e) = backend.connect(address.to_string()) {
                            event!(Level::ERROR, "Connection error: {:?}", e);
                        }

                    }
                },
                ConnectionStatus::Connecting => {
                    ui.label(format!("Connecting to {}", address));
                }
                ConnectionStatus::Initializing => {
                    ui.label(format!("Initializing connection"));
                },
                ConnectionStatus::Connected => {
                    if ui.button("Disconnect").clicked() {
                        if let Err(e) = backend.disconnect() {
                            event!(Level::ERROR, "Disconnection error: {:?}", e);
                        }
                    }

                }
            }

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });
        // self.backend.tick();
    }
}
