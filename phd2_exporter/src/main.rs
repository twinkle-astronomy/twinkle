mod metrics;

use metrics::Metrics;

use clap::Parser;
use phd2::Phd2Connection;
use tokio::net::TcpStream;

#[derive(Parser, Debug)]
struct Args {
    /// Connection address and port.
    #[arg(default_value = "localhost:4400")]
    address: String,

    /// Listen address and port
    #[arg(short, long, default_value = "0.0.0.0:9187")]
    listen: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    prometheus_exporter::start(args.listen.parse().unwrap()).unwrap();

    let metrics = Metrics::new();

    let phd2: Phd2Connection<_> = TcpStream::connect(&args.address)
        .await
        .expect(format!("Connecting to '{}'", args.address).as_str())
        .into();
    metrics.async_run(phd2).await.unwrap();
}
