use std::{collections::BTreeMap, num::NonZeroUsize};

use hash40::Hash40;

pub struct BucketMap<V>(Vec<BTreeMap<Hash40, V>>);

impl<V> BucketMap<V> {
    fn bucket_for_hash(&self, hash: Hash40) -> usize {
        hash.0 as usize % self.0.len()
    }

    pub fn new(bucket_count: NonZeroUsize) -> Self {
        let bucket_count = bucket_count.get();
        let mut buckets = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            buckets.push(BTreeMap::new());
        }
        Self(buckets)
    }

    pub fn get(&self, hash: Hash40) -> Option<&V> {
        self.0[self.bucket_for_hash(hash)].get(&hash)
    }

    pub fn get_mut(&mut self, hash: Hash40) -> Option<&mut V> {
        let bucket = self.bucket_for_hash(hash);
        self.0[bucket].get_mut(&hash)
    }

    pub fn insert(&mut self, hash: Hash40, value: V) -> Option<V> {
        let bucket = self.bucket_for_hash(hash);
        self.0[bucket].insert(hash, value)
    }

    pub fn remove(&mut self, hash: Hash40) -> Option<V> {
        let bucket = self.bucket_for_hash(hash);
        self.0[bucket].remove(&hash)
    }

    pub fn contains_key(&self, hash: Hash40) -> bool {
        self.get(hash).is_some()
    }

    pub fn into_inner(self) -> Vec<BTreeMap<Hash40, V>> {
        self.0
    }

    pub fn bucket_count(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Hash40, &V)> {
        self.0.iter().flat_map(|map| map.iter())
    }

    pub fn buckets(&self) -> impl Iterator<Item = &BTreeMap<Hash40, V>> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.iter().map(|map| map.len()).sum()
    }
}
