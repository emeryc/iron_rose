use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use iron_rose::IBF;

pub fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    for size in [100, 1000, 10000, 100000, 1000000].iter() {
        let mut ibf = IBF::new(50);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                for i in 0..size {
                    let _ = ibf.encode(black_box(i));
                }
            })
        });
    }
    group.finish();
}

criterion_group!(benches, encode);
criterion_main!(benches);
