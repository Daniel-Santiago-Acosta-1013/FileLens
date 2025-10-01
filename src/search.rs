use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use walkdir::WalkDir;

pub fn find_files(filename: &str) -> Vec<PathBuf> {
    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let search_paths = vec![
        home_dir.clone(),
        format!("{}/Documents", home_dir),
        format!("{}/Downloads", home_dir),
        format!("{}/Desktop", home_dir),
    ];

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["▹▹▹", "▸▹▹", "▹▸▹", "▹▹▸", "▹▹▹"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(120));

    let mut results = Vec::new();

    for search_path in search_paths {
        let path_name = search_path
            .replace(&home_dir, "~")
            .split('/')
            .next_back()
            .unwrap_or("")
            .to_string();
        spinner.set_message(
            style(format!("Buscando en {}...", path_name))
                .dim()
                .to_string(),
        );

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

    spinner.finish_and_clear();

    results.sort();
    results.dedup();
    results
}

pub fn find_directories(dir_name: &str) -> Vec<PathBuf> {
    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());

    let search_paths = vec![
        home_dir.clone(),
        format!("{}/Documents", home_dir),
        format!("{}/Downloads", home_dir),
        format!("{}/Desktop", home_dir),
    ];

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["▹▹▹", "▸▹▹", "▹▸▹", "▹▹▸", "▹▹▹"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(120));

    let mut results = Vec::new();

    for search_path in search_paths {
        let path_name = search_path
            .replace(&home_dir, "~")
            .split('/')
            .next_back()
            .unwrap_or("")
            .to_string();
        spinner.set_message(
            style(format!("Buscando en {}...", path_name))
                .dim()
                .to_string(),
        );

        let matches: Vec<PathBuf> = WalkDir::new(&search_path)
            .max_depth(15)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_type().is_dir()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(dir_name)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

        results.extend(matches);
    }

    spinner.finish_and_clear();

    results.sort();
    results.dedup();
    results
}
