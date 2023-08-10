use std::io::Write;
mod metrics;

use metrics::Metrics;

use clap::Parser;
use phd2::Phd2Connection;
use tokio::net::TcpStream;
use tokio_util::io::{InspectReader, InspectWriter};

#[derive(Parser, Debug)]
struct Args {
    /// Connection address and port.
    #[arg(default_value = "localhost:4400")]
    address: String,

    /// Listen address and port
    #[arg(short, long, default_value = "0.0.0.0:9187")]
    listen: String,

    /// Verbose logging.
    #[arg(short, long)]
    verbose: bool,
}

fn verbose_log(prefix: &str, buf: &[u8]) {
    std::io::stdout().write(prefix.as_bytes()).unwrap();
    std::io::stdout()
        .write(format!("{:?}", std::str::from_utf8(buf).unwrap()).as_bytes())
        .unwrap();
    std::io::stdout().write(&[b'\n']).unwrap();
}

fn regular_log(_p: &str, _b: &[u8]) {}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    prometheus_exporter::start(args.listen.parse().unwrap()).unwrap();

    let metrics = Metrics::new();

    let mf = if args.verbose {
        verbose_log
    } else {
        regular_log
    };

    let phd2: Phd2Connection<_> = TcpStream::connect(&args.address)
        .await
        .expect(format!("Connecting to '{}'", args.address).as_str())
        .inspect_read(move |buf: &[u8]| mf("-> ", buf))
        .inspect_write(move |buf: &[u8]| mf("<- ", buf))
        .into();
    metrics.async_run(phd2).await.unwrap();
}

pub trait WithInspectReader<R: tokio::io::AsyncRead> {
    fn inspect_read<F>(self, func: F) -> InspectReader<R, F>
    where
        F: FnMut(&[u8]);
}

impl<T: tokio::io::AsyncRead> WithInspectReader<T> for T {
    fn inspect_read<F>(self, func: F) -> InspectReader<T, F>
    where
        F: FnMut(&[u8]),
    {
        InspectReader::new(self, func)
    }
}

pub trait WithInspectWriter<R: tokio::io::AsyncWrite> {
    fn inspect_write<F>(self, func: F) -> InspectWriter<R, F>
    where
        F: FnMut(&[u8]);
}

impl<T: tokio::io::AsyncWrite> WithInspectWriter<T> for T {
    fn inspect_write<F>(self, func: F) -> InspectWriter<T, F>
    where
        F: FnMut(&[u8]),
    {
        InspectWriter::new(self, func)
    }
}
