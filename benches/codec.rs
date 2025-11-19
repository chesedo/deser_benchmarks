use std::hint::black_box;

use codec_comparison::{generate_test_data, ArchivedBlock, Block, FullTerm};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rkyv::{deserialize, rancor::Error};

fn measure_rkyv_size() {
    let test_data = generate_test_data();

    let mut total_size = 0;
    for block in &test_data {
        let bytes = rkyv::to_bytes::<Error>(block).unwrap();
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
            for block in black_box(&test_data) {
                let bytes = rkyv::to_bytes::<Error>(block).unwrap();
                black_box(bytes);
            }
        });
    });

    // Pre-serialize data for deserialization benchmarks
    let serialized_blocks: Vec<_> = test_data
        .iter()
        .map(|block| rkyv::to_bytes::<Error>(block).unwrap())
        .collect();

    // Benchmark full sequential read with full deserialization
    group.bench_function("full_read", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&serialized_blocks) {
                // Full deserialization
                let block = rkyv::from_bytes::<Block, Error>(serialized_block).unwrap();

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

    // Helper function to create a query mask that will match ~target_rate of entries
    fn create_query_mask(target_rate: f64) -> u128 {
        // More bits set = higher hit rate
        let num_bits = (128.0 * target_rate).round() as u32;
        if num_bits >= 128 {
            !0u128
        } else if num_bits == 0 {
            1u128 // At least one bit
        } else {
            (1u128 << num_bits) - 1
        }
    }

    // Benchmark filtered reads at different hit rates
    for hit_rate in [0.1, 0.5, 0.9] {
        let query_mask = create_query_mask(hit_rate);

        group.bench_with_input(
            BenchmarkId::new("filtered_read", format!("{}%", (hit_rate * 100.0) as u32)),
            &query_mask,
            |b, &query_mask| {
                b.iter(|| {
                    let mut total_frequency = 0u64;
                    let mut matched_count = 0usize;

                    for serialized_block in black_box(&serialized_blocks) {
                        // Zero-copy access to archived data
                        let archived =
                            rkyv::access::<ArchivedBlock, Error>(serialized_block).unwrap();

                        // Check each term's field_mask and only fully deserialize if it matches
                        for archived_term in archived.full_terms.iter() {
                            // Access field_mask without deserialization (zero-copy)
                            let field_mask = archived_term.field_mask;

                            if field_mask & query_mask != 0 {
                                // Only now do we "deserialize" by reading the other fields
                                let term = deserialize::<FullTerm, Error>(archived_term).unwrap();
                                let _doc_id = term.doc_id;
                                let _field_mask = term.field_mask;
                                total_frequency += term.frequency;
                                matched_count += 1;
                            }
                        }
                    }

                    (total_frequency, matched_count)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_rkyv_encoding);
criterion_main!(benches);
