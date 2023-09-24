use log::debug;
use phd2::{
    serialization::{Event, ServerEvent},
    Phd2Connection,
};
use prometheus_exporter::prometheus::{
    exponential_buckets, histogram_opts, linear_buckets, opts, register_gauge_vec,
    register_histogram_vec,
};
use tokio::sync::broadcast::error::RecvError;

pub struct Metrics {
    // guide_distance: GenericGaugeVec<AtomicF64>,
    guide_snr: prometheus_exporter::prometheus::GaugeVec,
    guide_snr_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_star_mass: prometheus_exporter::prometheus::GaugeVec,
    guide_star_mass_histo: prometheus_exporter::prometheus::HistogramVec,

    guide_hfd: prometheus_exporter::prometheus::GaugeVec,
    guide_hfd_histo: prometheus_exporter::prometheus::HistogramVec,

    ra_distance_raw: prometheus_exporter::prometheus::GaugeVec,
    de_distance_raw: prometheus_exporter::prometheus::GaugeVec,

    total_distance_raw: prometheus_exporter::prometheus::GaugeVec,
    total_distance_raw_histo: prometheus_exporter::prometheus::HistogramVec,

    pixel_scale: prometheus_exporter::prometheus::GaugeVec,
}

impl Metrics {
    pub fn new() -> Self {
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
                "Histogram of guid star mass",
                exponential_buckets(10_000.0, 1.1, 50).unwrap()
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
                linear_buckets(1.0, 0.1, 50).unwrap()
            ),
            &["host", "mount",]
        )
        .unwrap();

        let ra_distance_raw = register_gauge_vec!(
            opts!(
                "phd2_ra_distance_raw",
                "The RA distance in pixels of the guide offset vector"
            ),
            &["host", "mount",]
        )
        .unwrap();

        let de_distance_raw = register_gauge_vec!(
            opts!(
                "phd2_de_distance_raw",
                "The DEC distance in pixels of the guide offset vector"
            ),
            &["host", "mount",]
        )
        .unwrap();

        let total_distance_raw = register_gauge_vec!(
            opts!(
                "phd2_total_distance_raw",
                "The total distance in pixels of the guide offset vector"
            ),
            &["host", "mount",]
        )
        .unwrap();

        let total_distance_raw_histo = register_histogram_vec!(
            histogram_opts!(
                "phd2_total_distance_raw_histo",
                "Histogram of the total distance in pixels of the guide offset vector",
                exponential_buckets(0.01, 1.1, 100).unwrap()
            ),
            &["host", "mount",]
        )
        .unwrap();

        let pixel_scale = register_gauge_vec!(
            opts!("phd2_pixel_scale", "Guider image scale in arc-sec/pixel."),
            &[]
        )
        .unwrap();

        Metrics {
            guide_snr,
            guide_snr_histo,
            guide_star_mass,
            guide_star_mass_histo,
            guide_hfd,
            guide_hfd_histo,
            ra_distance_raw,
            de_distance_raw,
            total_distance_raw,
            total_distance_raw_histo,
            pixel_scale,
        }
    }

    fn handle_event(&self, event: &ServerEvent) {
        match &event.event {
            Event::GuideStep(guide) => {
                let snr = guide.snr;
                // dbg!(snr);
                self.guide_snr
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(snr);

                self.guide_snr_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(snr);

                let star_mass = guide.star_mass;
                // dbg!(star_mass);
                self.guide_star_mass
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(star_mass);

                self.guide_star_mass_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(star_mass);

                let hfd = guide.hfd;
                // dbg!(hfd);
                self.guide_hfd
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(hfd);

                self.guide_hfd_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(hfd);

                let ra_distance_raw = guide.ra_distance_raw;
                self.ra_distance_raw
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(ra_distance_raw);

                let de_distance_raw = guide.de_distance_raw;
                self.de_distance_raw
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(de_distance_raw);

                let total_distance_raw = (guide.ra_distance_raw * guide.ra_distance_raw
                    + guide.de_distance_raw * guide.de_distance_raw)
                    .sqrt();
                self.total_distance_raw
                    .with_label_values(&[&event.host, &guide.mount])
                    .set(total_distance_raw);

                self.total_distance_raw_histo
                    .with_label_values(&[&event.host, &guide.mount])
                    .observe(total_distance_raw);
            }
            _ => {}
        }
    }

    pub async fn async_run<T: Send + tokio::io::AsyncRead + tokio::io::AsyncWrite>(
        self,
        mut connection: Phd2Connection<T>,
    ) -> Result<(), tokio::sync::broadcast::error::RecvError> {
        let mut recv = match connection.subscribe().await {
            Some(recv) => recv,
            None => return Err(tokio::sync::broadcast::error::RecvError::Closed),
        };

        if let Ok(scale) = connection.get_pixel_scale().await {
            self.pixel_scale.with_label_values(&[]).set(scale);
        }

        loop {
            let event = recv.recv().await;
            debug!(target: "phd2_events", "{:?}", &event);
            match event {
                Ok(event) => {
                    self.handle_event(&event);

                    match &event.event {
                        Event::ConfigurationChange(_) => {
                            if let Ok(scale) = connection.get_pixel_scale().await {
                                self.pixel_scale.with_label_values(&[]).set(scale);
                            }
                        }
                        _ => {}
                    }
                }
                Err(RecvError::Closed) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }
}
