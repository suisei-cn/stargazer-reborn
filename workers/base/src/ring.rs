use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::ops::RangeInclusive;

use consistent_hash_ring::{migrated_ranges, Ring as RawRing, RingBuilder};
use fnv::FnvBuildHasher;

/// A collection that has been migrated to another node.
pub struct Migrated<'a, Node, Key, Hasher> {
    src: Node,
    dst: Node,
    migrated_keys: RangeInclusive<u64>,
    keys: &'a HashMap<Key, u64, Hasher>,
}

impl<'a, Node, Key, Hasher> Migrated<'a, Node, Key, Hasher> {
    /// The source node this hash range maps to.
    pub const fn src(&self) -> &Node {
        &self.src
    }
    /// The destination node this hash range maps to.
    pub const fn dst(&self) -> &Node {
        &self.dst
    }
    /// Keys that was migrated.
    pub fn keys(&'a self) -> impl Iterator<Item = &'a Key> {
        self.keys
            .iter()
            .filter(|(_, hash)| self.migrated_keys.contains(hash))
            .map(|(key, _)| key)
    }
}

/// A consistent hash ring.
pub struct Ring<Node, Key, Hasher = FnvBuildHasher> {
    ring: RawRing<Node, Hasher>,
    keys: HashMap<Key, u64, Hasher>,
    hasher: Hasher,
}

impl<Node, Key, Hasher> Default for Ring<Node, Key, Hasher>
where
    Node: Hash + Eq + Clone,
    Hasher: BuildHasher + Default,
{
    fn default() -> Self {
        Self {
            ring: RingBuilder::new(Default::default()).build(),
            keys: Default::default(),
            hasher: Default::default(),
        }
    }
}

impl<Node, Key, Hasher> Deref for Ring<Node, Key, Hasher> {
    type Target = RawRing<Node, Hasher>;

    fn deref(&self) -> &Self::Target {
        &self.ring
    }
}

impl<Node, Key, Hasher> Ring<Node, Key, Hasher>
where
    Node: Clone + Hash + Eq,
    Key: Hash + Eq,
    Hasher: BuildHasher + Clone,
{
    /// Insert a node into the ring.
    ///
    /// Returns a list of set of migrated keys.
    pub fn insert_node(&mut self, node: Node) -> Vec<Migrated<Node, Key, Hasher>> {
        self.mutate(|ring| {
            ring.insert(node);
        })
    }

    /// Insert a node into the ring.
    ///
    /// Returns a list of set of migrated keys.
    pub fn remove_node(&mut self, node: &Node) -> Vec<Migrated<Node, Key, Hasher>> {
        self.mutate(|ring| {
            ring.remove(node);
        })
    }

    /// Insert a key into the ring.
    ///
    /// Returns the node that the key was inserted into, if there's one,
    /// i.e. if there's no node in the ring, returns `None`.
    pub fn insert_key(&mut self, key: Key) -> Option<&Node> {
        let hash = self.hash(&key);
        self.keys
            .insert(key, hash)
            .and_then(|hash| self.ring.try_get(hash))
    }

    /// Remove a key from the ring.
    ///
    /// Returns the node that the key was removed from, if there's one,
    /// i.e. if there's no node in the ring or the key doesn't exist, returns
    /// `None`.
    pub fn remove_key(&mut self, key: &Key) -> Option<&Node> {
        self.keys
            .remove(key)
            .and_then(|hash| self.ring.try_get(hash))
    }

    /// Returns keys that are in the ring.
    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.keys.keys()
    }

    /// Returns the wrapped ring.
    pub const fn inner(&self) -> &RawRing<Node, Hasher> {
        &self.ring
    }

    /// Mutate the ring and returns list of set of migrated keys.
    fn mutate(
        &mut self,
        f: impl FnOnce(&mut RawRing<Node, Hasher>),
    ) -> Vec<Migrated<Node, Key, Hasher>> {
        let old_ring = self.ring.clone();
        f(&mut self.ring);
        migrated_ranges(&old_ring, &self.ring)
            .map(|migrated| Migrated {
                src: migrated.src().clone(),
                dst: migrated.dst().clone(),
                migrated_keys: migrated.keys().clone(),
                keys: &self.keys,
            })
            .collect()
    }

    /// Hash given key using the hasher.
    fn hash<K: Hash>(&self, key: K) -> u64 {
        let mut digest = self.hasher.build_hasher();
        key.hash(&mut digest);
        digest.finish()
    }
}
