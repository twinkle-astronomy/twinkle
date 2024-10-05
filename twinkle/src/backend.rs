// #[derive(Clone)]
// pub enum ConnectionStatus {
//     Disconnected,
//     Connecting,
//     Initializing,
//     Connected,
// }

// struct DropFn {
//     func: Box<dyn Fn() -> ()>,
// }

// impl Drop for DropFn {
//     fn drop(&mut self) {
//         (self.func)();
//     }
// }
/*
pub struct Backend {
    client: Arc<Mutex<indi::Client>>,
    connection: Arc<Mutex<Option<indi::Connection>>>,
    connection_status: Arc<Mutex<ConnectionStatus>>,
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            client: Arc::new(Mutex::new(indi::Client::new())),
            connection: Arc::new(Mutex::new(None)),
            connection_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let mut locked_connection = self.connection.lock().unwrap();
        if let Some(connection) = &mut *locked_connection {
            _ = connection.disconnect();
        }
    }
}
impl Backend {
    pub fn write(&self, command: &indi::Command) -> Result<(), DeError>{
        let mut locked_connection = self.connection.lock().unwrap();
        if let Some(connection) = &mut *locked_connection {
            connection.write(command)?;
        }
        Ok(())
    }

    #[instrument(skip(self, ctx))]
    pub fn connect(&mut self, ctx: egui::Context, address: String) -> Result<(), indi::DeError> {
        let runtime_client = Arc::clone(&self.client);
        let runtime_connection = Arc::clone(&self.connection);
        let runtime_connection_status = Arc::clone(&self.connection_status);
        self.disconnect()?;

        thread::spawn(move || -> Result<(), DeError> {
            let guard_client = Arc::clone(&runtime_client);
            let guard_ctx = ctx.clone();
            let guard_status = Arc::clone(&runtime_connection_status);
            let _guard = DropFn {
                func: Box::new(move || {
                    {
                        let mut l = guard_status.lock().unwrap();
                        *l = ConnectionStatus::Disconnected;
                        event!(Level::INFO, "Done with connection");
                    }
                    {
                        let mut l = guard_client.lock().unwrap();
                        l.clear();
                    }
                    guard_ctx.request_repaint();
                }),
            };

            let iter = {
                let mut locked_connection = runtime_connection.lock().unwrap();
                if let Some(connection) = &mut *locked_connection {
                    _ = connection.disconnect();
                }

                event!(Level::INFO, "Connecting to {}", address);
                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Connecting;
                    ctx.request_repaint();
                }

                let mut connection = indi::Connection::new(&address)?;
                event!(Level::INFO, "Connected, requesting properties");
                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Initializing;
                    ctx.request_repaint();
                }

                connection.write(&indi::GetProperties {
                    version: indi::INDI_PROTOCOL_VERSION.to_string(),
                    device: None,
                    name: None,
                })?;

                {
                    let mut l = runtime_connection_status.lock().unwrap();
                    *l = ConnectionStatus::Connected;
                    ctx.request_repaint();
                }
                let iter = connection.iter()?;
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
                        ctx.request_repaint();
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

    pub fn get_status(&self) -> ConnectionStatus {
        let l = self.connection_status.lock().unwrap();
        l.clone()
    }

    pub fn get_client(&self) -> Arc<Mutex<indi::Client>> {
        self.client.clone()
    }

    #[instrument(skip(self))]
    pub fn disconnect(&mut self) -> Result<(), DeError> {
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
*/
