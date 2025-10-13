// benches/secure_memset.rs

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::env;
use std::time::Duration;

// Environment overrides (optional):
// - BENCH_SAMPLE_SIZE: usize (default 100)
// - BENCH_MEASUREMENT_TIME_SEC: u64 seconds (default 5)
// - BENCH_WARMUP_TIME_SEC: u64 seconds (default 1)
fn criterion_config_from_env() -> Criterion {
    let sample_size = env::var("BENCH_SAMPLE_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100usize);
    let measurement_secs = env::var("BENCH_MEASUREMENT_TIME_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5u64);
    let warmup_secs = env::var("BENCH_WARMUP_TIME_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1u64);

    Criterion::default()
        .sample_size(sample_size)
        .measurement_time(Duration::from_secs(measurement_secs))
        .warm_up_time(Duration::from_secs(warmup_secs))
}

use libnfc_rs::nfc_secure_memset;

fn bench_secure_memset(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfc_secure_memset");
    for &size in &[64usize, 4096usize] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &s| {
            b.iter(|| {
                let mut buf = vec![0xFFu8; s];
                let rc = unsafe { nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, s) };
                black_box(rc);
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = criterion_config_from_env();
    targets = bench_secure_memset
}
criterion_main!(benches);
