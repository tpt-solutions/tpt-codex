// SPDX-License-Identifier: MIT OR Apache-2.0
use std::fs;
use std::path::PathBuf;

use tpt_codex_app::{DocumentStore, FileStorage, PersistError};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, MemoryStorage, Storage, TextStyle};

/// A fresh, empty directory under the system temp dir, unique per test.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tpt-codex-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.metadata.title = Some("Notes".into());
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "Hello".into(),
            style: TextStyle {
                bold: true,
                italic: false,
                heading_level: Some(1),
                runs: Vec::new(),
            },
        },
    );
    doc
}

#[test]
fn memory_round_trip_preserves_document() {
    let mut store = DocumentStore::new(MemoryStorage::new());
    let doc = sample();
    store.save("notes", &doc).unwrap();
    assert_eq!(store.load("notes").unwrap(), doc);
}

#[test]
fn file_round_trip_preserves_document() {
    let dir = scratch_dir("file-round-trip");
    let mut store = DocumentStore::new(FileStorage::new(&dir));
    let doc = sample();
    store.save("notes", &doc).unwrap();
    assert!(dir.join("documents").join("notes.json").exists());
    assert_eq!(store.load("notes").unwrap(), doc);
    // No temp file is left behind after a successful write.
    assert!(!dir.join("documents").join("notes.json.tmp").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_overwrites_and_delete_removes() {
    let mut store = DocumentStore::new(MemoryStorage::new());
    let mut doc = sample();
    store.save("notes", &doc).unwrap();
    doc.metadata.title = Some("Changed".into());
    store.save("notes", &doc).unwrap();
    assert_eq!(
        store.load("notes").unwrap().metadata.title.as_deref(),
        Some("Changed")
    );
    store.delete("notes").unwrap();
    store.delete("notes").unwrap();
    assert!(matches!(
        store.load("notes"),
        Err(PersistError::NotFound(_))
    ));
}

#[test]
fn invalid_documents_are_never_written() {
    let mut store = DocumentStore::new(MemoryStorage::new());
    let mut doc = sample();
    doc.root = BlockId(99);
    assert!(matches!(
        store.save("bad", &doc),
        Err(PersistError::Invalid(_))
    ));
    assert_eq!(store.storage().keys().count(), 0);
}

#[test]
fn invalid_documents_on_disk_are_rejected_on_load() {
    let mut storage = MemoryStorage::new();
    // A document with a dangling child, written by hand.
    let mut doc = sample();
    doc.insert(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(77)],
        },
    );
    storage
        .write("documents/broken.json", &serde_json::to_vec(&doc).unwrap())
        .unwrap();
    let store = DocumentStore::new(storage);
    assert!(matches!(
        store.load("broken"),
        Err(PersistError::Invalid(_))
    ));
}

#[test]
fn corrupt_json_is_reported_not_panicked() {
    let mut storage = MemoryStorage::new();
    storage.write("documents/junk.json", b"{ not json").unwrap();
    let store = DocumentStore::new(storage);
    assert!(matches!(store.load("junk"), Err(PersistError::Json(_))));
}

#[test]
fn names_are_restricted() {
    let mut store = DocumentStore::new(MemoryStorage::new());
    let doc = sample();
    for bad in ["", "../escape", "a/b", "with space", "dot.name"] {
        assert!(
            matches!(store.save(bad, &doc), Err(PersistError::InvalidName(_))),
            "{bad:?} should be rejected"
        );
    }
    store.save("ok_name-1", &doc).unwrap();
}

#[test]
fn file_storage_refuses_unsafe_keys() {
    let dir = scratch_dir("unsafe-keys");
    let mut storage = FileStorage::new(&dir);
    assert!(storage.write("../outside.json", b"x").is_err());
    assert!(storage.write("/abs.json", b"x").is_err());
    assert!(storage.read("").is_err());
    assert!(!dir.parent().unwrap().join("outside.json").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn file_storage_missing_key_reads_none() {
    let dir = scratch_dir("missing-key");
    let storage = FileStorage::new(&dir);
    assert_eq!(storage.read("nothing/here.json").unwrap(), None);
    fs::remove_dir_all(&dir).ok();
}
