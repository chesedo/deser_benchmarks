//! Manual zero-copy serialization implementation V3
//!
//! This version is optimized for full deserialization only using split_at pattern.
//! No zero-copy readers - just fast full deserialization.

use std::mem::size_of;

use crate::{Block, FullTerm};

const TERM_SIZE: usize = 32; // 8 + 16 + 8 bytes

/// Serialize a block to bytes using manual zero-copy layout
pub fn serialize(block: &Block) -> Vec<u8> {
    let num_terms = block.full_terms.len();
    let total_size = 4 + (num_terms * TERM_SIZE); // 4 bytes for length + terms

    let mut bytes = Vec::with_capacity(total_size);

    // Write number of terms as u32 little-endian
    bytes.extend_from_slice(&(num_terms as u32).to_le_bytes());

    // Write each term
    for term in &block.full_terms {
        bytes.extend_from_slice(&term.doc_id.to_le_bytes());
        bytes.extend_from_slice(&term.field_mask.to_le_bytes());
        bytes.extend_from_slice(&term.frequency.to_le_bytes());
    }

    bytes
}

/// Deserialize a block from bytes (full deserialization using split_at pattern)
pub fn deserialize(bytes: &[u8]) -> Result<Block, &'static str> {
    if bytes.len() < 4 {
        return Err("Buffer too small for header");
    }

    // Split off the header
    let (num_terms_bytes, mut remaining) = bytes.split_at(size_of::<u32>());

    // SAFETY: We checked the buffer size above
    let num_terms =
        unsafe { u32::from_le_bytes(num_terms_bytes.try_into().unwrap_unchecked()) } as usize;

    let expected_size = num_terms * TERM_SIZE;
    if remaining.len() < expected_size {
        return Err("Buffer too small for data");
    }

    let mut full_terms = Vec::with_capacity(num_terms);

    for _ in 0..num_terms {
        // Split off doc_id
        let (doc_id_bytes, rest) = remaining.split_at(size_of::<u64>());
        let doc_id = unsafe { u64::from_le_bytes(doc_id_bytes.try_into().unwrap_unchecked()) };
        remaining = rest;

        // Split off field_mask
        let (field_mask_bytes, rest) = remaining.split_at(size_of::<u128>());
        let field_mask =
            unsafe { u128::from_le_bytes(field_mask_bytes.try_into().unwrap_unchecked()) };
        remaining = rest;

        // Split off frequency
        let (frequency_bytes, rest) = remaining.split_at(size_of::<u64>());
        let frequency =
            unsafe { u64::from_le_bytes(frequency_bytes.try_into().unwrap_unchecked()) };
        remaining = rest;

        full_terms.push(FullTerm {
            doc_id,
            field_mask,
            frequency,
        });
    }

    Ok(Block { full_terms })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let block = Block {
            full_terms: vec![
                FullTerm {
                    doc_id: 1,
                    field_mask: 0xDEADBEEF,
                    frequency: 42,
                },
                FullTerm {
                    doc_id: 2,
                    field_mask: 0xCAFEBABE,
                    frequency: 123,
                },
            ],
        };

        let bytes = serialize(&block);
        let deserialized = deserialize(&bytes).unwrap();

        assert_eq!(block.full_terms.len(), deserialized.full_terms.len());
        assert_eq!(
            block.full_terms[0].doc_id,
            deserialized.full_terms[0].doc_id
        );
        assert_eq!(
            block.full_terms[1].frequency,
            deserialized.full_terms[1].frequency
        );
    }
}
