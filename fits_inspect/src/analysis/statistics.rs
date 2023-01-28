use ndarray::ArrayViewD;

pub struct Sample {
    pub value: u16,
    pub count: usize,
}

pub struct Statistics {
    pub unique: usize,
    pub median: u16,
    pub mean: f32,
    pub mad: u16,
    pub std_dev: f32,
    pub clip_high: Sample,
    pub clip_low: Sample,
    pub histogram: Vec<usize>,
}

impl Statistics {
    pub fn new(data: &ArrayViewD<u16>) -> Statistics {
        let histogram = Statistics::create_histogram(data);
        let median = Statistics::calculate_median(data, &histogram);

        let abs_dev = data.map(|x| median.abs_diff(*x));
        let abs_dev_histo = Statistics::create_histogram(&abs_dev.view());
        let mad = Statistics::calculate_median(&abs_dev.view(), &abs_dev_histo);

        let unique = histogram
            .iter()
            .map(|&item| if item > 0 { 1 } else { 0 })
            .sum();

        let clip_high = histogram
            .iter()
            .rev()
            .enumerate()
            .find_map(|(val, count)| {
                if *count == 0 {
                    return None;
                }

                Some(Sample {
                    value: std::u16::MAX - (val + 1) as u16,
                    count: *count,
                })
            })
            .unwrap_or_else(|| Sample {
                value: std::u16::MAX,
                count: 0,
            });

        let clip_low = histogram
            .iter()
            .enumerate()
            .find_map(|(val, count)| {
                if *count == 0 {
                    return None;
                }

                Some(Sample {
                    value: val as u16,
                    count: *count,
                })
            })
            .unwrap_or_else(|| Sample { value: 0, count: 0 });

        let mean = histogram
            .iter()
            .enumerate()
            .map(|(val, count)| (val as f32) * (*count as f32) / data.len() as f32)
            .sum();

        let std_dev = histogram
            .iter()
            .enumerate()
            .map(|(val, count)| (*count as f32) * ((val as f32) - mean) * ((val as f32) - mean))
            .sum::<f32>()
            .sqrt()
            / (data.shape().iter().product::<usize>() as f32);

        Statistics {
            unique,
            median,
            mean,
            mad,
            std_dev,
            clip_high,
            clip_low,
            histogram,
        }
    }

    fn calculate_median(data: &ArrayViewD<u16>, histogram: &Vec<usize>) -> u16 {
        let median_count: usize = data.shape().iter().product();
        let median = {
            let mut seen = 0;
            let mut median = 0;
            for (index, count) in histogram.iter().enumerate() {
                seen += *count;
                if seen >= median_count / 2 {
                    median = index;
                    break;
                }
            }
            median
        } as u16;
        median
    }

    fn create_histogram(data: &ArrayViewD<u16>) -> Vec<usize> {
        let mut histogram: Vec<usize> = vec![0; std::u16::MAX as usize];

        for d in data.iter() {
            histogram[*d as usize] += 1;
        }
        histogram
    }
}
