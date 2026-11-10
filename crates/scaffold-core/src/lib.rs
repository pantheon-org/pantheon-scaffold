//! Shared types of the pantheon-scaffold runner.
//!
//! The Rust runner must reproduce the TypeScript runner's behaviour (plan decision 7), so these types and their error
//! messages follow `runner/src/contract.ts` and `runner/src/manifest.ts`.

pub mod contract;
pub mod manifest;
pub mod pkl;
pub mod schema;
