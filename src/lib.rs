use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct FullTerm {
    pub doc_id: u64,
    pub field_mask: u128,
    pub frequency: u64,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Block {
    pub full_terms: Vec<FullTerm>,
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
