use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use rand::{
    thread_rng, Rng,
    distributions,
};


pub fn generate_rand_id(length: usize) -> String {
    thread_rng()
        .sample_iter(&distributions::Alphanumeric)
        .take(length)
        .collect()
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}