use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use quick_start_simple::{generate_test_data, Block};

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

    // Pre-serialize data for deserialization benchmarks
    let serialized_blocks: Vec<_> = test_data
        .iter()
        .map(|block| rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap())
        .collect();

    // Benchmark full sequential read with full deserialization
    group.bench_function("full_read", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&serialized_blocks) {
                // Full deserialization
                let block =
                    rkyv::from_bytes::<Block, rkyv::rancor::Error>(serialized_block).unwrap();

                // Iterate through all entries and read all fields
                for term in &block.full_terms {
                    let _doc_id = term.doc_id;
                    let _field_mask = term.field_mask;
                    total_frequency += term.frequency;
                }
            }

            total_frequency
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_rkyv_encoding);
criterion_main!(benches);
