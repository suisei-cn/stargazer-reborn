#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use base::ring::tests::{Key, Node, TestRing};

#[derive(Debug, Arbitrary)]
enum Op {
    InsertNode(Node),
    RemoveNode(Node),
    InsertKey(Key),
    RemoveKey(Key),
}

fuzz_target!(|ops: Vec<Op>| {
    let mut ring = TestRing::default();
    for op in ops {
        if option_env!("FUZZ_TRACE").is_some() {
            eprintln!("{:?}", op);
        }
        match op {
            Op::InsertNode(node) => ring.insert_node(node),
            Op::RemoveNode(node) => ring.remove_node(node),
            Op::InsertKey(key) => ring.insert_key(key),
            Op::RemoveKey(key) => ring.remove_key(key),
        }
    }
});
