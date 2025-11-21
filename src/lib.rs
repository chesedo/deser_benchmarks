pub mod manual_zerocopy;
pub mod manual_zerocopy_v2;

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    Debug,
    Clone,
    bincode::Encode,
    bincode::Decode,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct FullTerm {
    pub doc_id: u64,
    pub field_mask: u128,
    pub frequency: u64,
}

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    Debug,
    Clone,
    bincode::Encode,
    bincode::Decode,
    serde::Serialize,
    serde::Deserialize,
)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Block {
    pub full_terms: Vec<FullTerm>,
}

// Cap'n Proto conversion helpers
impl Block {
    pub fn to_capnp(&self, builder: &mut capnp::message::Builder<capnp::message::HeapAllocator>) {
        let mut block_builder = builder.init_root::<block_capnp::block::Builder>();
        let mut terms_builder = block_builder
            .reborrow()
            .init_full_terms(self.full_terms.len() as u32);

        for (i, term) in self.full_terms.iter().enumerate() {
            let mut term_builder = terms_builder.reborrow().get(i as u32);
            term_builder.set_doc_id(term.doc_id);

            let mut mask_builder = term_builder.reborrow().init_field_mask();
            mask_builder.set_high((term.field_mask >> 64) as u64);
            mask_builder.set_low(term.field_mask as u64);

            term_builder.set_frequency(term.frequency);
        }
    }

    pub fn from_capnp(reader: block_capnp::block::Reader) -> capnp::Result<Self> {
        let terms_reader = reader.get_full_terms()?;
        let mut full_terms = Vec::with_capacity(terms_reader.len() as usize);

        for term_reader in terms_reader.iter() {
            let mask_reader = term_reader.get_field_mask()?;
            let field_mask =
                ((mask_reader.get_high() as u128) << 64) | (mask_reader.get_low() as u128);

            full_terms.push(FullTerm {
                doc_id: term_reader.get_doc_id(),
                field_mask,
                frequency: term_reader.get_frequency(),
            });
        }

        Ok(Block { full_terms })
    }
}

// Include the generated Cap'n Proto code
pub mod block_capnp {
    include!(concat!(env!("OUT_DIR"), "/block_capnp.rs"));
}

/// Generate test data with 1M entries across blocks of 100 entries each
pub fn generate_test_data() -> Vec<Block> {
    const TOTAL_ENTRIES: usize = 1_000_000;
    const ENTRIES_PER_BLOCK: usize = 100;
    const NUM_BLOCKS: usize = TOTAL_ENTRIES / ENTRIES_PER_BLOCK;

    // Use a deterministic seed for reproducibility
    let mut rng = Xorshift64::new(42);

    let mut blocks = Vec::with_capacity(NUM_BLOCKS);
    let mut current_doc_id = 0u64;

    for _ in 0..NUM_BLOCKS {
        let mut terms = Vec::with_capacity(ENTRIES_PER_BLOCK);

        for _ in 0..ENTRIES_PER_BLOCK {
            // Increment doc_id, occasionally with gaps
            current_doc_id += if rng.next() % 10 == 0 {
                rng.next() % 5 + 1 // Gap of 1-5
            } else {
                1 // Sequential
            };

            terms.push(FullTerm {
                doc_id: current_doc_id,
                field_mask: rng.next_u128(),
                frequency: rng.next() % 1000 + 1,
            });
        }

        blocks.push(Block { full_terms: terms });
    }

    blocks
}

// Simple, deterministic PRNG for test data generation
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_u128(&mut self) -> u128 {
        let high = self.next() as u128;
        let low = self.next() as u128;
        (high << 64) | low
    }
}
