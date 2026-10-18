use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use iron_rose::IBF;
use std::hint::black_box;

pub fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    for size in [100u64, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*size));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut ibf = IBF::new(50);
                for i in 0..size {
                    ibf.encode(black_box(i as u128));
                }
                ibf
            })
        });
    }
    group.finish();
}

pub fn decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");
    for diff in [10u64, 100, 1000, 10000].iter() {
        let (mut l, mut r) = (IBF::new(*diff as usize * 2), IBF::new(*diff as usize * 2));
        for i in 0..*diff {
            if i % 2 == 0 {
                l.encode(i as u128);
            } else {
                r.encode(i as u128);
            }
        }
        let ibf = (l - r).unwrap();
        group.throughput(Throughput::Elements(*diff));
        group.bench_with_input(BenchmarkId::from_parameter(diff), &ibf, |b, ibf| {
            b.iter(|| ibf.clone().decode().unwrap())
        });
    }
    group.finish();
}

criterion_group!(benches, encode, decode);
criterion_main!(benches);
