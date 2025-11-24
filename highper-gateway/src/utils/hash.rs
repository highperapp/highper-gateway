//! Fast hashing utilities

use ahash::AHasher;
use std::hash::{Hash, Hasher};

/// Fast hash function using AHash
pub fn fast_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}
