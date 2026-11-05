//! The licence texts travel with the published crates, so every crate carries a copy of them.
//!
//! Cargo packages only what is inside a crate's own directory, so the copies cannot be
//! symlinks and they cannot point at the ones at the root of the repository. This test keeps
//! the copies and the ones at the root the same.

use std::path::PathBuf;

/// The files that have to be the same in every crate and at the root of the repository.
const FILES: &[&str] = &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "LICENSE-BSD3",
    "LICENSE-0BSD",
    "NOTICE",
];

/// The directory of the crate the test runs in.
fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The directory of the repository.
fn root_dir() -> PathBuf {
    crate_dir().join("../..")
}

/// The other crate of the workspace.
fn other_crate_dir() -> PathBuf {
    root_dir().join("crates/pele-rs-cli")
}

#[test]
fn every_crate_carries_the_licence_files_of_the_root() {
    for name in FILES {
        let root = root_dir().join(name);
        let text =
            std::fs::read_to_string(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        for dir in [crate_dir(), other_crate_dir()] {
            let copy = dir.join(name);
            let copied = std::fs::read_to_string(&copy)
                .unwrap_or_else(|e| panic!("{}: {e}", copy.display()));
            assert_eq!(
                copied,
                text,
                "{} is not the same as {}",
                copy.display(),
                root.display()
            );
        }
    }
}

#[test]
fn the_licence_of_the_manifest_is_one_of_the_ones_offered() {
    // The crates take their licence from the workspace.
    let manifest = std::fs::read_to_string(root_dir().join("Cargo.toml")).unwrap();
    for name in ["MIT", "Apache-2.0", "BSD-3-Clause", "0BSD"] {
        assert!(
            manifest.contains(name),
            "the workspace manifest does not offer the {name} licence"
        );
    }
}
