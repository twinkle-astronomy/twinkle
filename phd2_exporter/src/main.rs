use std::{fs::File, net::TcpStream, thread, time::Duration};

use phd2_exporter::{serialization::Event, Connection};
use prometheus_exporter::prometheus::{
    histogram_opts, linear_buckets, opts, register_gauge_vec, register_histogram_vec,
};

use clap::Parser;
use serde_json::StreamDeserializer;

#[derive(Parser, Debug)]
struct Args {
    /// Connection address and port.
    #[arg(default_value = "localhost:4400")]
    address: String,

    /// Listen address and port
    #[arg(short, long, default_value = "0.0.0.0:9187")]
    listen: String,

    /// Log of server messages to use instead of connecting to phd2.  For debugging purposes only.
    #[arg(short, long)]
    debug_logfile: Option<String>,
}

struct Metrics<T: Connection> {
    phd2_connection: T,

    // guide_distance: GenericGaugeVec<AtomicF64>,
    guide_snr: prometheus_exporter::prometheus::GaugeVec,
    guide_snr_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_star_mass: prometheus_exporter::prometheus::GaugeVec,
    guide_star_mass_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_hfd: prometheus_exporter::prometheus::GaugeVec,
    guide_hfd_histo: prometheus_exporter::prometheus::HistogramVec,

    pause: bool,
}

impl<T: Connection> Metrics<T> {
    pub fn new(phd2_connection: T, pause: bool) -> Self {
        let guide_snr =
            register_gauge_vec!(opts!("phd2_guide_snr", "Guide snr"), &["host", "mount",]).unwrap();

        let guide_snr_histo = register_histogram_vec!(
            histogram_opts!(
                "phd2_guide_snr_histo",
                "Histogram of snr",
                linear_buckets(10.0, 5.0, 50).unwrap()
            ),
            &["host", "mount",]
        )
        .unwrap();

        let guide_star_mass = register_gauge_vec!(
            opts!("phd2_guide_star_mass", "Guide star_mass"),
            &["host", "mount",]
        )
        .unwrap();

        let guide_star_mass_histo = register_histogram_vec!(
            histogram_opts!(
                "phd2_guide_star_mass_histo",
                "Histogram of snr",
                linear_buckets(10.0, 5.0, 50).unwrap()
            ),
            &["host", "mount",]
        )
        .unwrap();

        let guide_hfd = register_gauge_vec!(
            opts!("phd2_guide_hfd", "Guide star_mass"),
            &["host", "mount",]
        )
        .unwrap();

        let guide_hfd_histo = register_histogram_vec!(
            histogram_opts!(
                "phd2_guide_hfd_histo",
                "Histogram of guide hfd",
                linear_buckets(10.0, 5.0, 50).unwrap()
            ),
            &["host", "mount",]
        )
        .unwrap();

        Metrics {
            phd2_connection,
            pause,
            guide_snr,
            guide_snr_histo,
            guide_star_mass,
            guide_star_mass_histo,
            guide_hfd,
            guide_hfd_histo,
        }
    }

    fn handle_event(&self, event: Event) {
        // println!("Event: {:?}", event);
        match event {
            Event::GuideStep(guide) => {
                let snr = guide.snr;
                dbg!(snr);
                self.guide_snr
                    .with_label_values(&[&guide.host, &guide.mount])
                    .set(snr);

                self.guide_snr_histo
                    .with_label_values(&[&guide.host, &guide.mount])
                    .observe(snr);

                let star_mass = guide.star_mass;
                dbg!(star_mass);
                self.guide_star_mass
                    .with_label_values(&[&guide.host, &guide.mount])
                    .set(star_mass);

                self.guide_star_mass_histo
                    .with_label_values(&[&guide.host, &guide.mount])
                    .observe(star_mass);

                let hfd = guide.hfd;
                dbg!(hfd);
                self.guide_hfd
                    .with_label_values(&[&guide.host, &guide.mount])
                    .set(hfd);

                self.guide_hfd_histo
                    .with_label_values(&[&guide.host, &guide.mount])
                    .observe(hfd);

                if self.pause {
                    thread::sleep(Duration::from_secs(1));
                }
            }
            _ => {}
        }
    }

    pub fn run(self) {
        let i = self.phd2_connection.iter();
        // let iter = i.into_iter();
        for event in i {
            let e = event.unwrap();
            self.handle_event(e);
        }
    }
}

fn main() {
    let args = Args::parse();

    prometheus_exporter::start(args.listen.parse().unwrap()).unwrap();

    if let Some(logfile) = args.debug_logfile {
        let stream = File::open(logfile).unwrap();
        let metrics = Metrics::new(stream, true);
        metrics.run();
    } else {
        let stream = TcpStream::connect(args.address).expect("Connecting to phd2");
        let metrics = Metrics::new(stream, false);
        metrics.run();
    }
}
