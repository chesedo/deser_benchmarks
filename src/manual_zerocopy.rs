//! Manual zero-copy serialization implementation
//!
//! This implementation uses simple binary layout with little-endian encoding.
//! The format is:
//! - Block header: u32 length (number of terms)
//! - Terms: array of FullTerm structs laid out sequentially
//!
//! Each FullTerm is:
//! - doc_id: u64 (8 bytes)
//! - field_mask: u128 (16 bytes)
//! - frequency: u64 (8 bytes)
//! Total: 32 bytes per term

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

/// Deserialize a block from bytes (full deserialization)
pub fn deserialize(bytes: &[u8]) -> Result<Block, &'static str> {
    if bytes.len() < 4 {
        return Err("Buffer too small for header");
    }

    let num_terms = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let expected_size = 4 + (num_terms * TERM_SIZE);

    if bytes.len() < expected_size {
        return Err("Buffer too small for data");
    }

    let mut full_terms = Vec::with_capacity(num_terms);
    let mut offset = 4;

    for _ in 0..num_terms {
        let doc_id = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let field_mask = u128::from_le_bytes(bytes[offset..offset + 16].try_into().unwrap());
        offset += 16;

        let frequency = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;

        full_terms.push(FullTerm {
            doc_id,
            field_mask,
            frequency,
        });
    }

    Ok(Block { full_terms })
}

/// Zero-copy reader for accessing block data without full deserialization
pub struct BlockReader<'a> {
    bytes: &'a [u8],
    num_terms: usize,
}

impl<'a> BlockReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < 4 {
            return Err("Buffer too small for header");
        }

        let num_terms = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let expected_size = 4 + (num_terms * TERM_SIZE);

        if bytes.len() < expected_size {
            return Err("Buffer too small for data");
        }

        Ok(BlockReader { bytes, num_terms })
    }

    pub fn len(&self) -> usize {
        self.num_terms
    }

    pub fn iter(&self) -> TermIterator<'a> {
        TermIterator {
            bytes: self.bytes,
            offset: 4,
            remaining: self.num_terms,
        }
    }
}

/// Iterator over terms in a block (zero-copy)
pub struct TermIterator<'a> {
    bytes: &'a [u8],
    offset: usize,
    remaining: usize,
}

impl<'a> Iterator for TermIterator<'a> {
    type Item = TermReader<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        let term = TermReader {
            bytes: self.bytes,
            offset: self.offset,
        };

        self.offset += TERM_SIZE;
        self.remaining -= 1;

        Some(term)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a> ExactSizeIterator for TermIterator<'a> {}

/// Zero-copy reader for a single term
pub struct TermReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> TermReader<'a> {
    /// Read doc_id without deserializing the entire term
    #[inline]
    pub fn doc_id(&self) -> u64 {
        u64::from_le_bytes(self.bytes[self.offset..self.offset + 8].try_into().unwrap())
    }

    /// Read field_mask without deserializing the entire term
    #[inline]
    pub fn field_mask(&self) -> u128 {
        u128::from_le_bytes(
            self.bytes[self.offset + 8..self.offset + 24]
                .try_into()
                .unwrap(),
        )
    }

    /// Read frequency without deserializing the entire term
    #[inline]
    pub fn frequency(&self) -> u64 {
        u64::from_le_bytes(
            self.bytes[self.offset + 24..self.offset + 32]
                .try_into()
                .unwrap(),
        )
    }

    /// Fully deserialize this term
    pub fn deserialize(&self) -> FullTerm {
        FullTerm {
            doc_id: self.doc_id(),
            field_mask: self.field_mask(),
            frequency: self.frequency(),
        }
    }
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

    #[test]
    fn test_zero_copy_reader() {
        let block = Block {
            full_terms: vec![FullTerm {
                doc_id: 100,
                field_mask: 0xFF00FF00,
                frequency: 7,
            }],
        };

        let bytes = serialize(&block);
        let reader = BlockReader::new(&bytes).unwrap();

        assert_eq!(reader.len(), 1);

        let term = reader.iter().next().unwrap();
        assert_eq!(term.doc_id(), 100);
        assert_eq!(term.field_mask(), 0xFF00FF00);
        assert_eq!(term.frequency(), 7);
    }
}
