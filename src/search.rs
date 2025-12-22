use std::env;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Clone, Copy)]
enum SearchKind {
    File,
    Directory,
}

fn find_entries(name: &str, kind: SearchKind) -> Vec<PathBuf> {
    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let search_paths = vec![
        home_dir.clone(),
        format!("{}/Documents", home_dir),
        format!("{}/Downloads", home_dir),
        format!("{}/Desktop", home_dir),
    ];

    let mut results = Vec::new();

    for search_path in search_paths {
        let matches: Vec<PathBuf> = WalkDir::new(&search_path)
            .max_depth(15)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let is_match = entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(name);
                match kind {
                    SearchKind::File => entry.file_type().is_file() && is_match,
                    SearchKind::Directory => entry.file_type().is_dir() && is_match,
                }
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

        results.extend(matches);
    }

    results.sort();
    results.dedup();
    results
}

pub fn find_files(filename: &str) -> Vec<PathBuf> {
    find_entries(filename, SearchKind::File)
}

pub fn find_directories(dir_name: &str) -> Vec<PathBuf> {
    find_entries(dir_name, SearchKind::Directory)
}

pub fn find_files_quiet(filename: &str) -> Vec<PathBuf> {
    find_entries(filename, SearchKind::File)
}

pub fn find_directories_quiet(dir_name: &str) -> Vec<PathBuf> {
    find_entries(dir_name, SearchKind::Directory)
}
