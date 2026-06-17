use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn sum_n(n: u64) -> u64 {
    (0..n).sum()
}

fn bench_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("example_group");
    group.bench_function("sum/1000", |b| b.iter(|| black_box(sum_n(black_box(1000)))));
    group.finish();
}

criterion_group!(benches, bench_sum);
criterion_main!(benches);
