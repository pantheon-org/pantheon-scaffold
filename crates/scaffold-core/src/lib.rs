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

pub mod apply;
pub mod check;
pub mod checksums;
pub mod detach;
pub mod equality;
pub mod freshness;
pub mod http;
pub mod init;
pub mod knownbad;
pub mod latest;
pub mod lockcheck;
pub mod migrate;
pub mod pins;
pub mod preflight;
pub mod presets;
pub mod render;
pub mod semver;
#[cfg(test)]
mod testutil;
pub mod upgrade;
pub mod versions;
