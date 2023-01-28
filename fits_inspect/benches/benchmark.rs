use criterion::{criterion_group, criterion_main, Criterion};
use fits_inspect::analysis::Statistics;
use fitsio::FitsFile;
use ndarray::ArrayD;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics");
    group.sample_size(40);

    let mut fptr =
        FitsFile::open("images/M_33_Light_Red_180_secs_2022-11-24T18-58-20_001.fits").unwrap();
    let hdu = fptr.primary_hdu().unwrap();
    let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

    group.bench_function("Statistics::new(big)", |b| {
        b.iter(|| Statistics::new(&data.view()))
    });

    let mut fptr = FitsFile::open("images/PSF.fit").unwrap();
    let hdu = fptr.primary_hdu().unwrap();
    let data: ArrayD<u16> = hdu.read_image(&mut fptr).unwrap();

    group.bench_function("Statistics::new(sml)", |b| {
        b.iter(|| Statistics::new(&data.view()))
    });
    group.finish();

    let mut _group = c.benchmark_group("filters");

    // group.bench_function("phd2_convolv", |b| b.iter(|| phd2_convolve(&data)));
    // group.bench_function("sobel", |b| b.iter(|| sobel(&data)));

    // group.bench_function("padded", |b| b.iter(|| data.padded(IxDyn(&[10, 10]), 0)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
