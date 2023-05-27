use std::{
    fs::File,
    io::Write,
    thread,
    time::{Duration, SystemTime},
};

use phd2_exporter::{
    async_middleware::WithAsyncMiddleware, metrics::Metrics, serialization::ServerEvent,
    Connection, IntoPhd2Connection, WithMiddleware,
};

use clap::Parser;
use tokio::io::AsyncWriteExt;

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

fn verbose_log(buf: &[u8]) {
    std::io::stdout().write(buf).unwrap();
}

fn regular_log(_b: &[u8]) {}

struct DelayIter<T: Iterator<Item = Result<ServerEvent, serde_json::Error>>> {
    started_at: SystemTime,
    iter_started_at: Option<f64>,
    iter: T,
}

impl<T> DelayIter<T>
where
    T: Iterator<Item = Result<ServerEvent, serde_json::Error>>,
{
    pub fn new(iter: T) -> Self {
        DelayIter {
            iter,
            started_at: SystemTime::now(),
            iter_started_at: None,
        }
    }
}

impl<T> Iterator for DelayIter<T>
where
    T: Iterator<Item = Result<ServerEvent, serde_json::Error>>,
{
    type Item = Result<ServerEvent, serde_json::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iter.next()?;

        if let Ok(next) = &next {
            if let Some(iter_started_at) = self.iter_started_at {
                let system_runtime = self.started_at.elapsed().unwrap().as_secs_f64();
                let log_runtime = next.timestamp - iter_started_at;

                let delay = log_runtime - system_runtime;
                if delay > 0.0 {
                    thread::sleep(Duration::from_secs(delay as u64));
                }
            } else {
                self.iter_started_at = Some(next.timestamp);
            }
        }

        Some(next)
    }
}

// #[tokio::main]
// async fn main() {
//     let args = Args::parse();
//     let mut con = tokio::net::TcpStream::connect(args.address).await.expect("Connecting").phd2();

//     let req = serialization::JsonRpcRequest {
//         id: 1,
//         method: String::from("get_exposure"),
//         params: vec![]
//     };

//     dbg!(&req);

//     // dbg!(con.call(&req).await);

// }

async fn async_log(buf: &[u8]) {
    tokio::io::stdout().write_all(buf).await.unwrap();
}

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

    if let Some(logfile) = args.debug_logfile {
        let iter = File::open(logfile).unwrap().middleware(mf).iter();
        metrics.run(DelayIter::new(iter));
    } else {
        let con = tokio::net::TcpStream::connect(&args.address)
            .await
            .expect(format!("Connecting to '{}'", args.address).as_str())
            .middleware(mf)
            .phd2();
        metrics.async_run(con).await.unwrap();
    }
}
