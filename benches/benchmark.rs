use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use simplefs::fs::FS;

fn from_elem(c: &mut Criterion) {
    static KB: usize = 1024;
    static MB: usize = 1024 * KB;
    let fs = FS::connect("test.fs");
    let mut group = c.benchmark_group("写入测试");
    for size in [
        KB,
        4 * KB,
        16 * KB,
        64 * KB,
        256 * KB,
        MB,
		4 * MB,
        16 * MB,
        64 * MB,
        256 * MB,
    ]
    .iter()
    {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let content = "1".repeat(size);
            let content = content.as_bytes();
            b.iter(|| fs.write("/", "bench.txt", content));
        });
    }
    group.finish();
}

criterion_group!(benches, from_elem);
criterion_main!(benches);
