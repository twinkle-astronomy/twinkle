mod config;
mod metrics;
use std::fs::File;

use log::{debug, error};
use metrics::Metrics;

use clap::Parser;
use phd2::Phd2Connection;
use tokio::net::TcpStream;

#[derive(Parser, Debug)]
struct Args {
    /// Filename of configuration file.
    #[arg(default_value = None)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::builder().parse_env("LOG").init();

    let config_filename = Args::parse().config;
    let config = match &config_filename {
        Some(filename) => {
            let file = File::open(filename);
            match file {
                Ok(file) => serde_yaml::from_reader(file).expect("Reading config file"),
                Err(err) => {
                    debug!(
                        "Unable to open config file, attempting to create one: {:?}",
                        err
                    );
                    let file: File = File::create(filename).expect("Creating default config file.");
                    let config: config::Config = Default::default();
                    serde_yaml::to_writer(file, &config).expect("Writing config file.");
                    config
                }
            }
        }
        None => Default::default(),
    };

    prometheus_exporter::start(config.server.listen.parse().unwrap()).unwrap();

    let metrics = Metrics::new();

    let phd2: Phd2Connection<_> = TcpStream::connect(&config.server.address)
        .await
        .expect(format!("Connecting to '{}'", config.server.address).as_str())
        .into();
    metrics.async_run(phd2).await.unwrap();
}
