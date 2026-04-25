// benches/canny_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use doc_quad::core::buffer::DocBuffer;
use doc_quad::edge::detector::EdgeDetector;

fn bench_canny_detection(c: &mut Criterion) {
    let width = 1920;
    let height = 1080;
    let data = vec![128u8; (width * height) as usize];
    let buffer = DocBuffer::new(&data, width, height, width).unwrap();

    c.bench_function("canny_1080p", |b| {
        b.iter(|| {
            EdgeDetector::detect(black_box(&buffer)).unwrap()
        })
    });
}

criterion_group!(benches, bench_canny_detection);
criterion_main!(benches);