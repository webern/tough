// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use tempfile::TempDir;
use test_utils::{dir_url, test_data};
use tough::{FilesystemTransport, Limits, Repository, Settings};

mod test_utils;

struct RepoPaths {
    root_path: PathBuf,
    datastore: TempDir,
    metadata_base_url: String,
    targets_base_url: String,
}

impl RepoPaths {
    fn new() -> Self {
        let base = test_data().join("tuf-reference-impl");
        RepoPaths {
            root_path: base.join("metadata").join("1.root.json"),
            datastore: TempDir::new().unwrap(),
            metadata_base_url: dir_url(base.join("metadata")),
            targets_base_url: dir_url(base.join("targets")),
        }
    }

    fn root(&self) -> File {
        File::open(&self.root_path).unwrap()
    }
}

fn load_tuf_reference_impl<'a>(paths: &'a mut RepoPaths) -> Repository<'a, FilesystemTransport> {
    Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: &mut paths.root(),
            datastore: paths.datastore.as_ref(),
            metadata_base_url: paths.metadata_base_url.as_str(),
            targets_base_url: paths.targets_base_url.as_str(),
            limits: Limits::default(),
        },
    )
    .unwrap()
}

/// Test that the repo.cache() function works when given a list of multiple targets.
#[test]
fn test_repo_cache_all_targets() {
    // load the reference_impl repo
    let mut repo_paths = RepoPaths::new();
    let repo = load_tuf_reference_impl(&mut repo_paths);

    // cache the repo for future use
    let destination = TempDir::new().unwrap();
    let metadata_desintaion = destination.as_ref().join("metadata");
    let targets_desintaion = destination.as_ref().join("targets");
    repo.cache(&metadata_desintaion, &targets_desintaion, None, true)
        .unwrap();

    // check that we can load the copied repo.
    let datastore = TempDir::new().unwrap();
    let metadata_base_url = dir_url(&metadata_desintaion);
    let targets_base_url = dir_url(&targets_desintaion);
    let copied_repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: repo_paths.root(),
            datastore: datastore.as_ref(),
            metadata_base_url: metadata_base_url.as_str(),
            targets_base_url: targets_base_url.as_str(),
            limits: Limits::default(),
        },
    )
    .unwrap();

    // the copied repo should have file1 and file2 (i.e. all of targets).

    let mut file_data = Vec::new();
    let file_size = copied_repo
        .read_target("file1.txt")
        .unwrap()
        .unwrap()
        .read_to_end(&mut file_data)
        .unwrap();
    assert_eq!(31, file_size);

    let mut file_data = Vec::new();
    let file_size = copied_repo
        .read_target("file2.txt")
        .unwrap()
        .unwrap()
        .read_to_end(&mut file_data)
        .unwrap();
    assert_eq!(39, file_size);
}

/// Test that the repo.cache() function works when given a list of multiple targets.
#[test]
fn test_repo_cache_list_of_two_targets() {
    // load the reference_impl repo
    let mut repo_paths = RepoPaths::new();
    let repo = load_tuf_reference_impl(&mut repo_paths);

    // cache the repo for future use
    let destination = TempDir::new().unwrap();
    let metadata_desintaion = destination.as_ref().join("metadata");
    let targets_desintaion = destination.as_ref().join("targets");
    let targets_subset = vec!["file1.txt".to_string(), "file2.txt".to_string()];
    repo.cache(
        &metadata_desintaion,
        &targets_desintaion,
        Some(&targets_subset),
        true,
    )
    .unwrap();

    // check that we can load the copied repo.
    let datastore = TempDir::new().unwrap();
    let metadata_base_url = dir_url(&metadata_desintaion);
    let targets_base_url = dir_url(&targets_desintaion);
    let copied_repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: repo_paths.root(),
            datastore: datastore.as_ref(),
            metadata_base_url: metadata_base_url.as_str(),
            targets_base_url: targets_base_url.as_str(),
            limits: Limits::default(),
        },
    )
    .unwrap();

    // the copied repo should have file1 and file2 (i.e. all of the listed targets).

    let mut file_data = Vec::new();
    let file_size = copied_repo
        .read_target("file1.txt")
        .unwrap()
        .unwrap()
        .read_to_end(&mut file_data)
        .unwrap();
    assert_eq!(31, file_size);

    let mut file_data = Vec::new();
    let file_size = copied_repo
        .read_target("file2.txt")
        .unwrap()
        .unwrap()
        .read_to_end(&mut file_data)
        .unwrap();
    assert_eq!(39, file_size);
}

/// Test that the repo.cache() function works when given a list of only one of the targets.
#[test]
fn test_repo_cache_some() {
    // load the reference_impl repo
    let mut repo_paths = RepoPaths::new();
    let repo = load_tuf_reference_impl(&mut repo_paths);

    // cache the repo for future use
    let destination = TempDir::new().unwrap();
    let metadata_desintaion = destination.as_ref().join("metadata");
    let targets_desintaion = destination.as_ref().join("targets");
    let targets_subset = vec!["file2.txt".to_string()];
    repo.cache(
        &metadata_desintaion,
        &targets_desintaion,
        Some(&targets_subset),
        true,
    )
    .unwrap();

    // check that we can load the copied repo.
    let datastore = TempDir::new().unwrap();
    let metadata_base_url = dir_url(&metadata_desintaion);
    let targets_base_url = dir_url(&targets_desintaion);
    let copied_repo = Repository::load(
        &tough::FilesystemTransport,
        Settings {
            root: repo_paths.root(),
            datastore: datastore.as_ref(),
            metadata_base_url: metadata_base_url.as_str(),
            targets_base_url: targets_base_url.as_str(),
            limits: Limits::default(),
        },
    )
    .unwrap();

    // the copied repo should have file2 but not file1 (i.e. only the listed targets).

    let read_target_result = copied_repo.read_target("file1.txt");
    assert!(read_target_result.is_err());

    let mut file_data = Vec::new();
    let file_size = copied_repo
        .read_target("file2.txt")
        .unwrap()
        .unwrap()
        .read_to_end(&mut file_data)
        .unwrap();
    assert_eq!(39, file_size);
}
