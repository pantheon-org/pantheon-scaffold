//! Support for the tests of what the pantheon-scaffold package renders (Phase 7 of `docs/ts-tooling-rust-port.md`). The tests are in
//! `tests/`. They read the package in `pkl/` and the fixtures in `runner/test/fixtures`, which move into this crate in the last step of
//! the phase. Nothing here is shipped, and it holds no product code, so it is left out of mutation testing.

pub mod cli;
pub mod pkg;
pub mod repo;
pub mod serve;

pub use scaffold_core::equality::compare_directives;
pub use scaffold_core::equality::{compare_workflows, parse_yaml};

/// Parses TOML text.
pub fn parse_toml(text: &str) -> toml::Table {
    text.parse().expect("the TOML parses")
}
