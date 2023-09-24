use prometheus_exporter::prometheus::Error;
use prometheus_exporter::prometheus::{exponential_buckets, linear_buckets};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum HistogramBuckets {
    Linear {
        start: f64,
        width: f64,
        count: usize,
    },
    Exponential {
        start: f64,
        factor: f64,
        count: usize,
    },
    List(Vec<f64>),
}

impl TryFrom<HistogramBuckets> for Vec<f64> {
    type Error = Error;

    fn try_from(value: HistogramBuckets) -> std::result::Result<Self, Self::Error> {
        match value {
            HistogramBuckets::Linear {
                start,
                width,
                count,
            } => linear_buckets(start, width, count),
            HistogramBuckets::Exponential {
                start,
                factor,
                count,
            } => exponential_buckets(start, factor, count),
            HistogramBuckets::List(v) => Ok(v),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Metrics {
    guide_snr_histo: HistogramBuckets,
    guide_star_mass_histo: HistogramBuckets,
    guide_hfd_histo: HistogramBuckets,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            guide_snr_histo: HistogramBuckets::Linear {
                start: 10.0,
                width: 5.0,
                count: 50,
            },
            guide_star_mass_histo: HistogramBuckets::Exponential {
                start: 10_000.0,
                factor: 1.1,
                count: 50,
            },
            guide_hfd_histo: HistogramBuckets::Linear {
                start: 1.0,
                width: 0.1,
                count: 50,
            },
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Server {
    pub address: String,
    pub listen: String,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            address: String::from("localhost:4400"),
            listen: String::from("0.0.0.0:9187"),
        }
    }
}
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Config {
    pub server: Server,
    pub metrics: Metrics,
}

#[cfg(test)]
mod test {
    use super::Config;

    #[test]
    fn test_yaml() {
        let config: Config = Default::default();
        let config_string = serde_yaml::to_string(&config).unwrap();
        println!("{}", config_string);
        assert_eq!(config_string, String::default());
    }
}
