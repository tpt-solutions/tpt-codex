// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Clock, FixedClock, MemoryStorage, Storage};

#[test]
fn fixed_clock_is_constant() {
    let clock = FixedClock(1_700_000_000_000);
    assert_eq!(clock.now_millis(), 1_700_000_000_000);
    assert_eq!(clock.now_millis(), clock.now_millis());
}

#[test]
fn memory_storage_round_trip_and_remove() {
    let mut store = MemoryStorage::new();
    assert_eq!(store.read("doc").unwrap(), None);
    store.write("doc", b"one").unwrap();
    store.write("doc", b"two").unwrap();
    assert_eq!(store.read("doc").unwrap(), Some(b"two".to_vec()));
    store.write("a", b"x").unwrap();
    assert_eq!(store.keys().collect::<Vec<_>>(), vec!["a", "doc"]);
    store.remove("doc").unwrap();
    store.remove("missing").unwrap();
    assert_eq!(store.read("doc").unwrap(), None);
}
