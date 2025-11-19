use std::hint::black_box;

use capnp::message::{Builder, ReaderOptions};
use codec_comparison::{block_capnp, generate_test_data, ArchivedBlock, Block, FullTerm};
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

    // Measure capnp size
    let mut capnp_size = 0;
    for block in &test_data {
        let mut message = Builder::new_default();
        block.to_capnp(&mut message);
        let bytes = capnp::serialize::write_message_to_words(&message);
        capnp_size += bytes.len() * 8; // words are 8 bytes each
    }
    print_size_stats("capnp", capnp_size);

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

fn benchmark_serialize(c: &mut Criterion) {
    let test_data = generate_test_data();
    let bincode_config = bincode::config::standard();

    let mut group = c.benchmark_group("serialize");

    group.bench_function("rkyv", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap();
                black_box(bytes);
            }
        });
    });

    group.bench_function("bincode", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = bincode::encode_to_vec(block, bincode_config).unwrap();
                black_box(bytes);
            }
        });
    });

    group.bench_function("postcard", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let bytes = postcard::to_stdvec(block).unwrap();
                black_box(bytes);
            }
        });
    });

    group.bench_function("capnp", |b| {
        b.iter(|| {
            for block in black_box(&test_data) {
                let mut message = Builder::new_default();
                block.to_capnp(&mut message);
                let bytes = capnp::serialize::write_message_to_words(&message);
                black_box(bytes);
            }
        });
    });

    group.finish();
}

fn benchmark_full_read(c: &mut Criterion) {
    let test_data = generate_test_data();
    let bincode_config = bincode::config::standard();

    // Pre-serialize data for all formats
    let rkyv_blocks: Vec<_> = test_data
        .iter()
        .map(|block| rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap())
        .collect();

    let bincode_blocks: Vec<_> = test_data
        .iter()
        .map(|block| bincode::encode_to_vec(block, bincode_config).unwrap())
        .collect();

    let postcard_blocks: Vec<_> = test_data
        .iter()
        .map(|block| postcard::to_stdvec(block).unwrap())
        .collect();

    let capnp_blocks: Vec<_> = test_data
        .iter()
        .map(|block| {
            let mut message = Builder::new_default();
            block.to_capnp(&mut message);
            capnp::serialize::write_message_to_words(&message)
        })
        .collect();

    let mut group = c.benchmark_group("full_read");

    group.bench_function("rkyv", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&rkyv_blocks) {
                let block =
                    rkyv::from_bytes::<Block, rkyv::rancor::Error>(serialized_block).unwrap();

                for term in &block.full_terms {
                    let _doc_id = term.doc_id;
                    let _field_mask = term.field_mask;
                    total_frequency += term.frequency;
                }
            }

            total_frequency
        });
    });

    group.bench_function("bincode", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&bincode_blocks) {
                let (block, _len): (Block, usize) =
                    bincode::decode_from_slice(serialized_block, bincode_config).unwrap();

                for term in &block.full_terms {
                    let _doc_id = term.doc_id;
                    let _field_mask = term.field_mask;
                    total_frequency += term.frequency;
                }
            }

            total_frequency
        });
    });

    group.bench_function("postcard", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&postcard_blocks) {
                let block: Block = postcard::from_bytes(serialized_block).unwrap();

                for term in &block.full_terms {
                    let _doc_id = term.doc_id;
                    let _field_mask = term.field_mask;
                    total_frequency += term.frequency;
                }
            }

            total_frequency
        });
    });

    group.bench_function("capnp", |b| {
        b.iter(|| {
            let mut total_frequency = 0u64;

            for serialized_block in black_box(&capnp_blocks) {
                let reader = capnp::serialize::read_message_from_flat_slice(
                    &mut &serialized_block[..],
                    ReaderOptions::new(),
                )
                .unwrap();
                let block =
                    Block::from_capnp(reader.get_root::<block_capnp::block::Reader>().unwrap())
                        .unwrap();

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

fn benchmark_filtered_read(c: &mut Criterion) {
    let test_data = generate_test_data();
    let bincode_config = bincode::config::standard();

    // Pre-serialize data for all formats
    let rkyv_blocks: Vec<_> = test_data
        .iter()
        .map(|block| rkyv::to_bytes::<rkyv::rancor::Error>(block).unwrap())
        .collect();

    let bincode_blocks: Vec<_> = test_data
        .iter()
        .map(|block| bincode::encode_to_vec(block, bincode_config).unwrap())
        .collect();

    let postcard_blocks: Vec<_> = test_data
        .iter()
        .map(|block| postcard::to_stdvec(block).unwrap())
        .collect();

    let capnp_blocks: Vec<_> = test_data
        .iter()
        .map(|block| {
            let mut message = Builder::new_default();
            block.to_capnp(&mut message);
            capnp::serialize::write_message_to_words(&message)
        })
        .collect();

    for hit_rate in [0.1, 0.5, 0.9] {
        let query_mask = create_query_mask(hit_rate);
        let group_name = format!("filtered_read_{}%", (hit_rate * 100.0) as u32);
        let mut group = c.benchmark_group(&group_name);

        group.bench_function("rkyv", |b| {
            b.iter(|| {
                let mut total_frequency = 0u64;
                let mut matched_count = 0usize;

                for serialized_block in black_box(&rkyv_blocks) {
                    let archived =
                        rkyv::access::<ArchivedBlock, rkyv::rancor::Error>(serialized_block)
                            .unwrap();

                    for archived_term in archived.full_terms.iter() {
                        let field_mask = archived_term.field_mask;

                        if field_mask & query_mask != 0 {
                            let term =
                                rkyv::deserialize::<FullTerm, rkyv::rancor::Error>(archived_term)
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
        });

        group.bench_function("bincode", |b| {
            b.iter(|| {
                let mut total_frequency = 0u64;
                let mut matched_count = 0usize;

                for serialized_block in black_box(&bincode_blocks) {
                    let (block, _len): (Block, usize) =
                        bincode::decode_from_slice(serialized_block, bincode_config).unwrap();

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
        });

        group.bench_function("postcard", |b| {
            b.iter(|| {
                let mut total_frequency = 0u64;
                let mut matched_count = 0usize;

                for serialized_block in black_box(&postcard_blocks) {
                    let block: Block = postcard::from_bytes(serialized_block).unwrap();

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
        });

        group.bench_function("capnp", |b| {
            b.iter(|| {
                let mut total_frequency = 0u64;
                let mut matched_count = 0usize;

                for serialized_block in black_box(&capnp_blocks) {
                    let reader = capnp::serialize::read_message_from_flat_slice(
                        &mut &serialized_block[..],
                        ReaderOptions::new(),
                    )
                    .unwrap();
                    let block_reader = reader.get_root::<block_capnp::block::Reader>().unwrap();
                    let terms_reader = block_reader.get_full_terms().unwrap();

                    for term_reader in terms_reader.iter() {
                        let mask_reader = term_reader.get_field_mask().unwrap();
                        let field_mask = ((mask_reader.get_high() as u128) << 64)
                            | (mask_reader.get_low() as u128);

                        if field_mask & query_mask != 0 {
                            let _doc_id = term_reader.get_doc_id();
                            let frequency = term_reader.get_frequency();
                            total_frequency += frequency;
                            matched_count += 1;
                        }
                    }
                }

                (total_frequency, matched_count)
            });
        });

        group.finish();
    }
}

fn all_benchmarks(c: &mut Criterion) {
    measure_sizes();
    benchmark_serialize(c);
    benchmark_full_read(c);
    benchmark_filtered_read(c);
}

criterion_group!(benches, all_benchmarks);
criterion_main!(benches);
