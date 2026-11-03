// SPDX-License-Identifier: MIT OR Apache-2.0
//! Trait boundaries for platform services.
//!
//! The core never talks to the operating system, the browser or the network
//! directly. Hosts implement these traits. The in-memory and fixed-clock
//! implementations here are for tests and headless use.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// A source of time, in milliseconds since an arbitrary epoch.
pub trait Clock {
    /// Current time in milliseconds.
    fn now_millis(&self) -> u64;
}

/// A clock that always returns the same time. Useful for deterministic output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        self.0
    }
}

/// Keyed byte storage supplied by the host (files, browser storage, a server).
pub trait Storage {
    /// The error this storage reports.
    type Error;

    /// Reads the bytes stored under `key`, or `None` if there are none.
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Stores `bytes` under `key`, replacing any previous value.
    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), Self::Error>;

    /// Removes `key`. Removing a key that does not exist is not an error.
    fn remove(&mut self, key: &str) -> Result<(), Self::Error>;
}

/// An in-memory [`Storage`] that never fails.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemoryStorage {
    entries: BTreeMap<String, Vec<u8>>,
}

impl MemoryStorage {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keys currently stored, in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

impl Storage for MemoryStorage {
    type Error = core::convert::Infallible;

    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.entries.get(key).cloned())
    }

    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), Self::Error> {
        self.entries.insert(String::from(key), bytes.to_vec());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), Self::Error> {
        self.entries.remove(key);
        Ok(())
    }
}
