use std::{thread, time::Duration};

use prometheus_exporter::prometheus::{register_gauge_vec, register_histogram_vec, opts, histogram_opts, linear_buckets};

use crate::{serialization::{ServerEvent, Event}, Connection};


pub struct Metrics {
    // guide_distance: GenericGaugeVec<AtomicF64>,
    guide_snr: prometheus_exporter::prometheus::GaugeVec,
    guide_snr_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_star_mass: prometheus_exporter::prometheus::GaugeVec,
    guide_star_mass_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_hfd: prometheus_exporter::prometheus::GaugeVec,
    guide_hfd_histo: prometheus_exporter::prometheus::HistogramVec,

    pause: bool,
}

impl Metrics {
    pub fn new(pause: bool) -> Self {
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
            pause,
            guide_snr,
            guide_snr_histo,
            guide_star_mass,
            guide_star_mass_histo,
            guide_hfd,
            guide_hfd_histo,
        }
    }

    fn handle_event(&self, event: ServerEvent) {
        // println!("Event: {:?}", event);
        match event.event {
            Event::GuideStep(guide) => {
                let snr = guide.snr;
                dbg!(snr);
                self.guide_snr
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(snr);

                self.guide_snr_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(snr);

                let star_mass = guide.star_mass;
                dbg!(star_mass);
                self.guide_star_mass
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(star_mass);

                self.guide_star_mass_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(star_mass);

                let hfd = guide.hfd;
                dbg!(hfd);
                self.guide_hfd
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(hfd);

                self.guide_hfd_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(hfd);

                if self.pause {
                    thread::sleep(Duration::from_secs(1));
                }
            }
            _ => {}
        }
    }

    pub fn run<T: Connection + std::io::Read>(self, phd2_connection: T) {
        let i = phd2_connection.iter();
        // let iter = i.into_iter();
        for event in i {
            let e = event.unwrap();
            self.handle_event(e);
        }
    }
}
