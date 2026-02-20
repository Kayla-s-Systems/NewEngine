// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::prelude::*;

#[test]
fn ne_hashmap_is_deterministic_for_same_insertion_order() {
    let mut a: NeHashMap<u32, u32> = NeHashMap::default();
    let mut b: NeHashMap<u32, u32> = NeHashMap::default();

    for i in 0..128u32 {
        a.insert(i, i.wrapping_mul(3));
        b.insert(i, i.wrapping_mul(3));
    }

    let keys_a: Vec<u32> = a.keys().copied().collect();
    let keys_b: Vec<u32> = b.keys().copied().collect();
    assert_eq!(keys_a, keys_b);
}

#[test]
fn untrusted_map_compiles_and_is_usable() {
    let mut m: UntrustedMap<u32, u32> = ne_untrusted_map();
    m.insert(1, 2);
    assert_eq!(m.get(&1).copied(), Some(2));
}

#[test]
fn btreemap_has_sorted_iteration_order() {
    let mut m: NeBTreeMap<u32, u32> = NeBTreeMap::new();
    m.insert(5, 0);
    m.insert(1, 0);
    m.insert(3, 0);

    let keys: Vec<u32> = m.keys().copied().collect();
    assert_eq!(keys, vec![1, 3, 5]);
}
