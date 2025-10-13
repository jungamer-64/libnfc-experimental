// benches/secure_copy.rs

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::env;
use std::time::Duration;

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

use libnfc_rs::{nfc_safe_memcpy, nfc_safe_memmove};

fn bench_secure_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfc_secure_copy");
    for &size in &[64usize, 1024usize, 4096usize] {
        // memcpy: non-overlapping buffers
        let src = vec![0xAAu8; size];
        let mut dst = vec![0u8; size];
        let src_ptr = src.as_ptr() as *const _;
        let dst_ptr = dst.as_mut_ptr() as *mut _;
        group.bench_with_input(
            BenchmarkId::new("memcpy_nonoverlap", size),
            &size,
            |b, &_s| {
                b.iter(|| unsafe {
                    let rc = nfc_safe_memcpy(dst_ptr as *mut _, size, src_ptr as *const _, size);
                    black_box(rc);
                })
            },
        );

        // memmove: non-overlapping (same as memcpy semantics)
        let src2 = vec![0xBBu8; size];
        let mut dst2 = vec![0u8; size];
        let src2_ptr = src2.as_ptr() as *const _;
        let dst2_ptr = dst2.as_mut_ptr() as *mut _;
        group.bench_with_input(
            BenchmarkId::new("memmove_nonoverlap", size),
            &size,
            |b, &_s| {
                b.iter(|| unsafe {
                    let rc = nfc_safe_memmove(dst2_ptr as *mut _, size, src2_ptr as *const _, size);
                    black_box(rc);
                })
            },
        );

        // memmove: overlapping buffers — multiple overlap offsets and both directions
        let mut buf = vec![0xCCu8; size * 2];
        let base = buf.as_mut_ptr();
        // Offsets: small, quarter, half, near-full overlap (when possible)
        let mut offsets = Vec::new();
        if size >= 2 {
            offsets.push(1usize);
        }
        if size / 4 >= 1 {
            offsets.push(size / 4);
        }
        if size / 2 >= 1 {
            offsets.push(size / 2);
        }
        if size >= 2 {
            offsets.push(size - 1);
        }
        offsets.sort_unstable();
        offsets.dedup();

        for &off in &offsets {
            // forward overlap: dst starts after src
            let src_ptr = base as *const _;
            let dst_ptr_fwd = unsafe { base.add(off) } as *mut _;
            let id_fwd = format!("memmove_overlap_fwd_{}", off);
            group.bench_with_input(BenchmarkId::new(id_fwd, size), &size, |b, &_s| {
                b.iter(|| unsafe {
                    let rc =
                        nfc_safe_memmove(dst_ptr_fwd as *mut _, size, src_ptr as *const _, size);
                    black_box(rc);
                })
            });

            // backward overlap: dst starts before src
            let dst_ptr_bwd = base as *mut _;
            let src_ptr_bwd = unsafe { base.add(off) } as *const _;
            let id_bwd = format!("memmove_overlap_bwd_{}", off);
            group.bench_with_input(BenchmarkId::new(id_bwd, size), &size, |b, &_s| {
                b.iter(|| unsafe {
                    let rc = nfc_safe_memmove(
                        dst_ptr_bwd as *mut _,
                        size,
                        src_ptr_bwd as *const _,
                        size,
                    );
                    black_box(rc);
                })
            });
        }
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = criterion_config_from_env();
    targets = bench_secure_copy
}
criterion_main!(benches);
