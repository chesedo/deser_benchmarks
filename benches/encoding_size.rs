use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use quick_start_simple::generate_test_data;

fn measure_rkyv_size() {
    let test_data = generate_test_data();

    let mut total_size = 0;
    for block in &test_data {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap();
        total_size += bytes.len();
    }

    println!("\n=== rkyv Encoding Size ===");
    println!(
        "Total size: {} bytes ({:.2} MB)",
        total_size,
        total_size as f64 / 1_048_576.0
    );
    println!(
        "Average per entry: {:.2} bytes",
        total_size as f64 / 1_000_000.0
    );
    println!(
        "Average per block: {:.2} bytes\n",
        total_size as f64 / 10_000.0
    );
}

fn benchmark_rkyv_encoding(c: &mut Criterion) {
    // Measure and print size once before benchmarks
    measure_rkyv_size();

    let test_data = generate_test_data();

    let mut group = c.benchmark_group("rkyv");

    // Benchmark serialization speed
    group.bench_function("serialize", |b| {
        b.iter(|| {
            let mut serialized_blocks = Vec::new();
            for block in black_box(&test_data) {
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap();
                serialized_blocks.push(bytes);
            }
            serialized_blocks
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_rkyv_encoding);
criterion_main!(benches);
