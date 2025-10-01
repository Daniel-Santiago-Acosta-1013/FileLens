use std::fs::{self, Metadata};
use std::path::Path;

const DIRECTORY_COUNT_LIMIT: usize = 500;

#[derive(Clone)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl EntryKind {
    pub fn from(metadata: &Metadata) -> Self {
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            Self::Directory
        } else if file_type.is_file() {
            Self::File
        } else if file_type.is_symlink() {
            Self::Symlink
        } else {
            Self::Other
        }
    }
}

pub fn count_directory_entries(path: &Path) -> Result<(usize, bool), String> {
    let read_dir = fs::read_dir(path).map_err(|error| {
        format!(
            "No se pudo contar los elementos de `{}`: {error}",
            path.display()
        )
    })?;

    let mut count = 0;
    let mut truncated = false;

    for entry in read_dir.take(DIRECTORY_COUNT_LIMIT + 1) {
        if entry.is_ok() {
            count += 1;
        }
    }

    if count > DIRECTORY_COUNT_LIMIT {
        truncated = true;
        count = DIRECTORY_COUNT_LIMIT;
    }

    Ok((count, truncated))
}
