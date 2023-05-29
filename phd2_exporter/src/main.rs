use std::{io::Write, time::Duration};

use phd2_exporter::{metrics::Metrics, Phd2Connection};

use clap::Parser;
use tokio::net::TcpStream;
use tokio_util::io::{InspectReader, InspectWriter};

// struct DelayIter<T: Iterator<Item = Result<ServerEvent, serde_json::Error>>> {
//     started_at: SystemTime,
//     iter_started_at: Option<f64>,
//     iter: T,
// }

// impl<T> DelayIter<T>
// where
//     T: Iterator<Item = Result<ServerEvent, serde_json::Error>>,
// {
//     pub fn new(iter: T) -> Self {
//         DelayIter {
//             iter,
//             started_at: SystemTime::now(),
//             iter_started_at: None,
//         }
//     }
// }

// impl<T> Iterator for DelayIter<T>
// where
//     T: Iterator<Item = Result<ServerEvent, serde_json::Error>>,
// {
//     type Item = Result<ServerEvent, serde_json::Error>;

//     fn next(&mut self) -> Option<Self::Item> {
//         let next = self.iter.next()?;

//         if let Ok(next) = &next {
//             if let Some(iter_started_at) = self.iter_started_at {
//                 let system_runtime = self.started_at.elapsed().unwrap().as_secs_f64();
//                 let log_runtime = next.timestamp - iter_started_at;

//                 let delay = log_runtime - system_runtime;
//                 if delay > 0.0 {
//                     thread::sleep(Duration::from_secs(delay as u64));
//                 }
//             } else {
//                 self.iter_started_at = Some(next.timestamp);
//             }
//         }

//         Some(next)
//     }
// }

#[derive(Parser, Debug)]
struct Args {
    /// Connection address and port.
    #[arg(default_value = "localhost:4400")]
    address: String,

    /// Listen address and port
    #[arg(short, long, default_value = "0.0.0.0:9187")]
    listen: String,

    /// Use named log of server messages to use instead of connecting to phd2.  For debugging purposes only.
    /// To generate this log file use `nc <PHD2 IP> 4400 | tee phd2-events-"`date +"%d-%m-%YT%H-%M-%S"`".log`
    #[arg(short, long)]
    debug_logfile: Option<String>,

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

    let mut phd2: Phd2Connection<_> = TcpStream::connect(&args.address)
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
