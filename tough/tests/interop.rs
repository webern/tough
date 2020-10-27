// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use test_utils::{dir_url, read_to_end, test_data};
use tough::{ExpirationEnforcement, Limits, Repository, Settings};

mod test_utils;

/// Test that `tough` can process repositories generated by [`tuf`], the reference Python
/// implementation.
///
/// [`tuf`]: https://github.com/theupdateframework/tuf
#[test]
fn test_tuf_reference_impl() {
    let base = test_data().join("tuf-reference-impl");

    let repo = Repository::load(
        Box::new(tough::FilesystemTransport),
        Settings {
            root: File::open(base.join("metadata").join("1.root.json")).unwrap(),
            datastore: None,
            metadata_base_url: dir_url(base.join("metadata")),
            targets_base_url: dir_url(base.join("targets")),
            limits: Limits::default(),
            expiration_enforcement: ExpirationEnforcement::Safe,
        },
    )
    .unwrap();

    assert_eq!(
        read_to_end(repo.read_target("file1.txt").unwrap().unwrap()),
        &b"This is an example target file."[..]
    );
    assert_eq!(
        read_to_end(repo.read_target("file2.txt").unwrap().unwrap()),
        &b"This is an another example target file."[..]
    );
    assert_eq!(
        repo.targets()
            .signed
            .targets
            .get("file1.txt")
            .unwrap()
            .custom
            .get("file_permissions")
            .unwrap(),
        "0644"
    );

    assert!(repo
        .targets()
        .signed
        .delegations
        .as_ref()
        .unwrap()
        .target_is_delegated(&"file3.txt".to_string()));
}
