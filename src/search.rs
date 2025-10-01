use std::env;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn find_files(filename: &str) -> Vec<PathBuf> {
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
                entry.file_type().is_file()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(filename)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

        results.extend(matches);
    }

    results.sort();
    results.dedup();
    results
}
