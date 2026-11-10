//! Shared types of the pantheon-scaffold runner.
//!
//! The Rust runner must reproduce the TypeScript runner's behaviour (plan decision 7), so these types and their error
//! messages follow `runner/src/contract.ts` and `runner/src/manifest.ts`.

pub mod contract;
pub mod guard;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod pkl;
pub mod schema;

pub mod check;
pub mod equality;
pub mod render;
#[cfg(test)]
mod testutil;
pub mod versions;
