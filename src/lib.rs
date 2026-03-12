//! BMO — local-first SQLite-backed issue tracker.
//!
//! This crate is primarily used via the `bmo` CLI binary. The library surface
//! exposes the database layer, domain models, error types, and the Axum-based
//! web server for embedding or programmatic use.
//!
//! The `filter` module is internal to the binary and is not re-exported here.

// Dead code is expected during incremental development; remove before 1.0
#![allow(dead_code)]

pub mod db;
pub mod errors;
pub mod model;
pub mod web;
