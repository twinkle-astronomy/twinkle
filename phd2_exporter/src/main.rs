use std::{fs::File, io::Write, net::TcpStream};

use phd2_exporter::{
    metrics::Metrics,
    WithMiddleware,
};

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Connection address and port.
    #[arg(default_value = "localhost:4400")]
    address: String,

    /// Listen address and port
    #[arg(short, long, default_value = "0.0.0.0:9187")]
    listen: String,

    /// Use named log of server messages to use instead of connecting to phd2.  For debugging purposes only.
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

fn main() {
    let args = Args::parse();

    prometheus_exporter::start(args.listen.parse().unwrap()).unwrap();

    let metrics = Metrics::new(args.debug_logfile.is_some());

    let mf = if args.verbose {
        verbose_log
    } else {
        regular_log
    };

    if let Some(logfile) = args.debug_logfile {
        let stream = File::open(logfile).unwrap().middleware(mf);
        metrics.run(stream);
    } else {
        let stream = TcpStream::connect(args.address)
            .expect("Connecting to phd2")
            .middleware(mf);
        metrics.run(stream);
    }
}
