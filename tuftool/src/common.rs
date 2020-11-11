/// This module if for code that is re-used by different `tuftool` subcommands.
use crate::error::{self, Result};
use snafu::ResultExt;
use std::fs::File;
use std::path::Path;
use tough::{Repository, Settings};

/// Some commands only deal with metadata and never use a targets directory.
/// When loading a repo that does not need a targets directory, we pass this as
/// the targets URL.
pub(crate) const UNUSED_URL: &str = "file:///unused/url";

/// Load a repo for metadata processing only. Such a repo will never use the
/// targets directory, so a dummy path is passed.
///
/// - `root` must be a path to a file that can be opened with `File::open`.
/// - `metadata_url` can be local or remote.
///
pub(crate) fn load_metadata_repo<P, S>(root: P, metdata_url: S) -> Result<Repository>
where
    P: AsRef<Path>,
    S: AsRef<str>,
{
    let root = root.as_ref();
    Repository::load_default(Settings {
        root: File::open(root).context(error::OpenRoot { path: root })?,
        metadata_base_url: metdata_url.as_ref(),
        // we don't do anything with the targets url for metadata operations
        targets_base_url: UNUSED_URL,
    })
    .context(error::RepoLoad)
}
