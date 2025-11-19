use std::hint::black_box;

use codec_comparison::{generate_test_data, ArchivedBlock, Block, FullTerm};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn print_size_stats(name: &str, total_size: usize) {
    println!(
        "\n{} total size: {} bytes ({:.2} MB)",
        name,
        total_size,
        total_size as f64 / 1_048_576.0
    );
    println!(
        "  Average per entry: {:.2} bytes",
        total_size as f64 / 1_000_000.0
    );
    println!(
        "  Average per block: {:.2} bytes",
        total_size as f64 / 10_000.0
    );
}

fn measure_sizes() {
    let test_data = generate_test_data();

    println!("\n=== Encoding Sizes ===");

    // Measure rkyv size
    let mut rkyv_size = 0;
    for block in &test_data {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap();
        rkyv_size += bytes.len();
    }
    print_size_stats("rkyv", rkyv_size);

    // Measure bincode size
    let bincode_config = bincode::config::standard();
    let mut bincode_size = 0;
    for block in &test_data {
        let bytes = bincode::encode_to_vec(block, bincode_config).unwrap();
        bincode_size += bytes.len();
    }
    print_size_stats("bincode", bincode_size);

    // Measure postcard size
    let mut postcard_size = 0;
    for block in &test_data {
        let bytes = postcard::to_stdvec(block).unwrap();
        postcard_size += bytes.len();
    }
    print_size_stats("postcard", postcard_size);

    println!(); // Extra newline after all sizes
}

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

fn benchmark_rkyv(c: &mut Criterion) {
    let test_data = generate_test_data();

    let mut group = c.benchmark_group("rkyv");

    // Benchmark serialization speed
    group.bench_function("serialize", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap();
                black_box(bytes);
            }
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
                            rkyv::access::<ArchivedBlock, rkyv::rancor::Error>(serialized_block)
                                .unwrap();

                        // Check each term's field_mask and only fully deserialize if it matches
                        for archived_term in archived.full_terms.iter() {
                            // Access field_mask without deserialization (zero-copy)
                            let field_mask = archived_term.field_mask;

                            if field_mask & query_mask != 0 {
                                // Only now do we "deserialize" by reading the other fields
                                let term = rkyv::deserialize::<FullTerm, rkyv::rancor::Error>(
                                    archived_term,
                                )
                                .unwrap();
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

fn benchmark_bincode(c: &mut Criterion) {
    let test_data = generate_test_data();
    let bincode_config = bincode::config::standard();

    let mut group = c.benchmark_group("bincode");

    // Benchmark serialization speed
    group.bench_function("serialize", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = bincode::encode_to_vec(block, bincode_config).unwrap();
                black_box(bytes);
            }
        });
    });

    // Pre-serialize data for deserialization benchmarks
    let serialized_blocks: Vec<_> = test_data
        .iter()
        .map(|block| bincode::encode_to_vec(block, bincode_config).unwrap())
        .collect();

    // Benchmark full sequential read with full deserialization
    group.bench_function("full_read", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&serialized_blocks) {
                // Full deserialization
                let (block, _len): (Block, usize) =
                    bincode::decode_from_slice(serialized_block, bincode_config).unwrap();

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
                        // bincode requires full deserialization
                        let (block, _len): (Block, usize) =
                            bincode::decode_from_slice(serialized_block, bincode_config).unwrap();

                        // Check each term's field_mask and only process if it matches
                        for term in &block.full_terms {
                            if term.field_mask & query_mask != 0 {
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

fn benchmark_postcard(c: &mut Criterion) {
    let test_data = generate_test_data();

    let mut group = c.benchmark_group("postcard");

    // Benchmark serialization speed
    group.bench_function("serialize", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = postcard::to_stdvec(block).unwrap();
                black_box(bytes);
            }
        });
    });

    // Pre-serialize data for deserialization benchmarks
    let serialized_blocks: Vec<_> = test_data
        .iter()
        .map(|block| postcard::to_stdvec(block).unwrap())
        .collect();

    // Benchmark full sequential read with full deserialization
    group.bench_function("full_read", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&serialized_blocks) {
                // Full deserialization
                let block: Block = postcard::from_bytes(serialized_block).unwrap();

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
                        // postcard requires full deserialization
                        let block: Block = postcard::from_bytes(serialized_block).unwrap();

                        // Check each term's field_mask and only process if it matches
                        for term in &block.full_terms {
                            if term.field_mask & query_mask != 0 {
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

fn all_benchmarks(c: &mut Criterion) {
    measure_sizes();
    benchmark_rkyv(c);
    benchmark_bincode(c);
    benchmark_postcard(c);
}

criterion_group!(benches, all_benchmarks);
criterion_main!(benches);
