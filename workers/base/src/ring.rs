use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::ops::{Deref, RangeInclusive};

use consistent_hash_ring::{migrated_ranges, Ring as RawRing, RingBuilder};
use fnv::FnvBuildHasher;

enum LightCow<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<'a, T> Deref for LightCow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            LightCow::Borrowed(t) => *t,
            LightCow::Owned(t) => t,
        }
    }
}

/// A collection that has been migrated to another node.
pub struct Migrated<'a, Node, Key, Hasher = FnvBuildHasher> {
    src: Node,
    dst: Node,
    migrated_keys: RangeInclusive<u64>,
    keys: LightCow<'a, HashMap<Key, u64, Hasher>>,
}

impl<'a, Node, Key, Hasher> Migrated<'a, Node, Key, Hasher>
where
    Node: Clone,
    Key: Eq + Hash + Clone,
    Hasher: BuildHasher + Default,
{
    pub fn to_owned(&self) -> Migrated<'static, Node, Key, Hasher> {
        Migrated {
            src: self.src.clone(),
            dst: self.dst.clone(),
            migrated_keys: self.migrated_keys.clone(),
            keys: LightCow::Owned(self.keys.iter().map(|(k, v)| (k.clone(), *v)).collect()),
        }
    }
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
    ///
    /// # Edge Case
    ///
    /// If the ring had keys but no nodes, you should insert all keys into this
    /// new node. The `Migrated` struct won't handle this for you.
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
        self.keys.insert(key, hash);
        self.ring.try_get(hash)
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
                keys: LightCow::Borrowed(&self.keys),
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

#[cfg(any(test, fuzzing))]
mod tests {
    use std::collections::{HashMap, HashSet};

    use consistent_hash_ring::RingBuilder;

    use super::Ring;
    use crate::ring::Migrated;

    #[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
    struct Node(u64);

    #[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
    struct Key(u64);

    #[derive(Default)]
    struct TestRing {
        ring: Ring<Node, Key>,
        buckets: HashMap<Node, HashSet<Key>>,
        keys: HashSet<Key>,
    }

    impl TestRing {
        fn insert_node(&mut self, node: Node) {
            if self.buckets.is_empty() {
                self.buckets.insert(node, self.keys.clone());
            } else {
                self.buckets.insert(node, HashSet::new());
            }

            let migration = self
                .ring
                .insert_node(node)
                .into_iter()
                .map(|migrated| migrated.to_owned())
                .collect();
            self.merge_migration(migration);

            Self::assert(
                self.buckets.keys().copied(),
                self.keys.iter().copied(),
                &self.buckets,
            );
        }
        fn remove_node(&mut self, node: Node) {
            let migration = self
                .ring
                .remove_node(&node)
                .into_iter()
                .map(|migrated| migrated.to_owned())
                .collect();
            self.merge_migration(migration);

            self.buckets.remove(&node).unwrap().is_empty();

            Self::assert(
                self.buckets.keys().copied(),
                self.keys.iter().copied(),
                &self.buckets,
            );
        }
        fn insert_key(&mut self, key: Key) {
            self.keys.insert(key);
            let rtn = self.ring.insert_key(key).copied();
            Self::assert_single(self.buckets.keys().copied(), key, rtn);
            if let Some(node) = rtn {
                self.buckets.get_mut(&node).unwrap().insert(key);
            }
        }
        fn remove_key(&mut self, key: Key) {
            self.keys.remove(&key);
            let rtn = self.ring.remove_key(&key).copied();
            Self::assert_single(self.buckets.keys().copied(), key, rtn);
            if let Some(node) = rtn {
                self.buckets.get_mut(&node).unwrap().remove(&key);
            }
        }

        fn merge_migration(&mut self, migration: Vec<Migrated<Node, Key>>) {
            for migrated in migration {
                for key in migrated.keys() {
                    self.buckets.get_mut(migrated.src()).unwrap().remove(key);
                    self.buckets.get_mut(migrated.dst()).unwrap().insert(*key);
                }
            }
        }

        fn assert_single(nodes: impl Iterator<Item = Node>, key: Key, actual: Option<Node>) {
            let ring = RingBuilder::default().nodes_iter(nodes).build();
            if ring.is_empty() {
                assert!(actual.is_none());
            } else {
                assert_eq!(actual, Some(*ring.get(&key)));
            }
        }

        #[track_caller]
        fn assert(
            nodes: impl Iterator<Item = Node> + Clone,
            keys: impl Iterator<Item = Key>,
            actual: &HashMap<Node, HashSet<Key>>,
        ) {
            let ring = RingBuilder::default().nodes_iter(nodes.clone()).build();
            if ring.is_empty() {
                assert!(actual.is_empty());
            } else {
                let mut expected = HashMap::new();
                for node in nodes {
                    expected.insert(node, HashSet::new());
                }
                for key in keys {
                    expected.get_mut(ring.get(key)).unwrap().insert(key);
                }
                assert_eq!(&expected, actual);
            }
        }
    }

    #[test]
    fn must_return_none() {
        let mut ring: Ring<Node, Key> = Ring::default();
        assert_eq!(ring.insert_key(Key(1)), None); // no node in the ring
        ring.insert_node(Node(1));
        assert_eq!(ring.remove_key(&Key(2)), None); // node doesn't exist
        ring.remove_node(&Node(1));
        assert_eq!(ring.remove_key(&Key(1)), None); // no node in the ring
    }

    #[test]
    fn must_consistent() {
        let mut test_ring = TestRing::default();
        test_ring.insert_key(Key(1));
        test_ring.insert_key(Key(1));
        test_ring.insert_key(Key(2));
        test_ring.remove_key(Key(1));
        test_ring.remove_key(Key(1));
        test_ring.remove_key(Key(2));

        test_ring.insert_node(Node(1));
        test_ring.insert_node(Node(1));
        test_ring.insert_node(Node(2));
        test_ring.remove_node(Node(1));
        test_ring.remove_node(Node(2));

        test_ring.insert_node(Node(1));
        test_ring.insert_key(Key(1));
        test_ring.insert_node(Node(2));
        test_ring.insert_node(Node(3));
        test_ring.insert_key(Key(1));
        test_ring.insert_key(Key(2));
        test_ring.remove_key(Key(1));
        test_ring.insert_node(Node(4));
        test_ring.insert_key(Key(3));
        test_ring.remove_node(Node(2));
        test_ring.insert_key(Key(4));
        test_ring.remove_node(Node(1));
        test_ring.remove_node(Node(3));
        test_ring.remove_node(Node(4));

        test_ring.insert_key(Key(5));
        test_ring.insert_node(Node(1));
    }
}
