# Serialization Deserialization Benchmarks

Comprehensive benchmarks comparing different Rust serialization libraries for a specific use case: blocks of inverted index terms with selective field filtering.

## Libraries Compared

- **rkyv** - Zero-copy deserialization with archived types
- **bincode** - Compact binary serialization
- **postcard** - Embedded-friendly serialization
- **Cap'n Proto** - Schema-based zero-copy serialization
- **manual_zerocopy** - Custom zero-copy implementation (v1: offset-based, v2: reference-based)

## Benchmark Scenarios

### 1. Encoding Size
Measures the serialized size for 1 million entries across 10,000 blocks.

### 2. Serialization Speed
Measures time to serialize all blocks.

### 3. Full Read
Measures full deserialization and sequential read of all fields.

### 4. Filtered Read (10%, 50%, 90% hit rates)
Measures performance when only deserializing entries matching a field mask filter. Zero-copy libraries (rkyv, capnp, manual implementations) can check the filter field without deserializing the entire entry.

## Data Structure
```rust
struct FullTerm {
    doc_id: u64,
    field_mask: u128,
    frequency: u64,
}

struct Block {
    full_terms: Vec<FullTerm>,  // 100 entries per block
}
```

## Running Benchmarks
```bash
cargo bench
```

Results will be saved to `target/criterion/`.

## Key Findings

Zero-copy deserialization (rkyv, capnp, manual implementations) shows significant advantages for filtered reads at low hit rates, where checking a single field before deserializing the rest provides substantial performance benefits.
