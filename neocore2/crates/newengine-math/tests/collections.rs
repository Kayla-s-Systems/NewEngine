#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::{BTreeMap, FxHashMap, SecureHashMap};

#[test]
fn fx_hashmap_is_deterministic_for_same_insertion_order() {
    let mut a: FxHashMap<u32, u32> = FxHashMap::default();
    let mut b: FxHashMap<u32, u32> = FxHashMap::default();

    for i in 0..128u32 {
        a.insert(i, i.wrapping_mul(3));
        b.insert(i, i.wrapping_mul(3));
    }

    let keys_a: Vec<u32> = a.keys().copied().collect();
    let keys_b: Vec<u32> = b.keys().copied().collect();
    assert_eq!(keys_a, keys_b);
}

#[test]
fn secure_hashmap_compiles_and_is_usable() {
    let mut m: SecureHashMap<u32, u32> = SecureHashMap::default();
    m.insert(1, 2);
    assert_eq!(m.get(&1).copied(), Some(2));
}

#[test]
fn btreemap_has_sorted_iteration_order() {
    let mut m: BTreeMap<u32, u32> = BTreeMap::new();
    m.insert(5, 0);
    m.insert(1, 0);
    m.insert(3, 0);

    let keys: Vec<u32> = m.keys().copied().collect();
    assert_eq!(keys, vec![1, 3, 5]);
}
