use filelens::metadata::renderer::build_report;
use filelens::metadata::report::MetadataOptions;
use filelens::metadata_editor::{
    analyze_directory as analyze_directory_core, analyze_files as analyze_files_core,
    apply_office_metadata_edit, collect_candidate_files, DirectoryAnalysisSummary,
    DirectoryFilter, filter_files, remove_all_metadata,
};
use filelens::search::{find_directories_quiet, find_files_quiet};
use rfd::FileDialog;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Emitter;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CleanupProgress {
    Started { total: usize },
    Processing { index: usize, total: usize, path: String },
    Success { path: String },
    Failure { path: String, error: String },
    Finished { successes: usize, failures: usize },
}

#[tauri::command]
fn analyze_file(path: String, include_hash: bool) -> Result<filelens::metadata::report::MetadataReport, String> {
    let options = MetadataOptions { include_hash };
    build_report(Path::new(&path), &options)
}

#[tauri::command]
fn analyze_directory(path: String, recursive: bool) -> Result<DirectoryAnalysisSummary, String> {
    analyze_directory_core(Path::new(&path), recursive)
}

#[tauri::command]
fn analyze_files(paths: Vec<String>) -> Result<DirectoryAnalysisSummary, String> {
    let files: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    analyze_files_core(&files)
}

#[tauri::command]
fn search_files(query: String) -> Result<Vec<String>, String> {
    let results = find_files_quiet(query.trim());
    Ok(results
        .into_iter()
        .map(|path| path.display().to_string())
        .collect())
}

#[tauri::command]
fn search_directories(query: String) -> Result<Vec<String>, String> {
    let results = find_directories_quiet(query.trim());
    Ok(results
        .into_iter()
        .map(|path| path.display().to_string())
        .collect())
}

#[tauri::command]
fn remove_metadata(path: String) -> Result<(), String> {
    remove_all_metadata(Path::new(&path))
}

#[tauri::command]
fn edit_office_metadata(path: String, field: String, value: String) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("El valor no puede estar vacio".to_string());
    }

    let tag = match field.to_lowercase().as_str() {
        "author" | "autor" => "dc:creator",
        "title" | "titulo" => "dc:title",
        "subject" | "asunto" => "dc:subject",
        "company" | "empresa" => "Company",
        _ => return Err("Campo no soportado".to_string()),
    };

    apply_office_metadata_edit(Path::new(&path), tag, value)
        .map_err(|err| format!("No se pudo actualizar la metadata: {}", err))
}

#[tauri::command]
fn pick_file() -> Option<String> {
    FileDialog::new()
        .pick_file()
        .map(|path| path.display().to_string())
}

#[tauri::command]
fn pick_directory() -> Option<String> {
    FileDialog::new()
        .pick_folder()
        .map(|path| path.display().to_string())
}

#[tauri::command]
fn pick_files() -> Option<Vec<String>> {
    FileDialog::new().pick_files().map(|paths| {
        paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect()
    })
}

#[tauri::command]
fn start_cleanup(
    app: tauri::AppHandle,
    path: String,
    recursive: bool,
    filter: String,
) -> Result<(), String> {
    let filter = parse_filter(&filter)?;
    let dir = PathBuf::from(path);
    let mut files = collect_candidate_files(&dir, recursive, filter)?;

    if files.is_empty() {
        return Err("No hay archivos compatibles para limpiar".to_string());
    }

    files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let total = files.len();
    let _ = app_handle.emit(
        "cleanup://progress",
        CleanupProgress::Started { total },
    );

        let mut successes = 0_usize;
        let mut failures = 0_usize;

        for (index, path) in files.into_iter().enumerate() {
            let _ = app_handle.emit(
                "cleanup://progress",
                CleanupProgress::Processing {
                    index: index + 1,
                    total,
                    path: path.display().to_string(),
                },
            );

            match remove_all_metadata(&path) {
                Ok(()) => {
                    successes += 1;
                    let _ = app_handle.emit(
                        "cleanup://progress",
                        CleanupProgress::Success {
                            path: path.display().to_string(),
                        },
                    );
                }
                Err(error) => {
                    failures += 1;
                    let _ = app_handle.emit(
                        "cleanup://progress",
                        CleanupProgress::Failure {
                            path: path.display().to_string(),
                            error,
                        },
                    );
                }
            }
        }

        let _ = app_handle.emit(
            "cleanup://progress",
            CleanupProgress::Finished { successes, failures },
        );
    });

    Ok(())
}

#[tauri::command]
fn start_cleanup_files(
    app: tauri::AppHandle,
    paths: Vec<String>,
    filter: String,
) -> Result<(), String> {
    let filter = parse_filter(&filter)?;
    let files: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let mut files = filter_files(&files, filter);

    if files.is_empty() {
        return Err("No hay archivos compatibles para limpiar".to_string());
    }

    files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let total = files.len();
        let _ = app_handle.emit(
            "cleanup://progress",
            CleanupProgress::Started { total },
        );

        let mut successes = 0_usize;
        let mut failures = 0_usize;

        for (index, path) in files.into_iter().enumerate() {
            let _ = app_handle.emit(
                "cleanup://progress",
                CleanupProgress::Processing {
                    index: index + 1,
                    total,
                    path: path.display().to_string(),
                },
            );

            match remove_all_metadata(&path) {
                Ok(()) => {
                    successes += 1;
                    let _ = app_handle.emit(
                        "cleanup://progress",
                        CleanupProgress::Success {
                            path: path.display().to_string(),
                        },
                    );
                }
                Err(error) => {
                    failures += 1;
                    let _ = app_handle.emit(
                        "cleanup://progress",
                        CleanupProgress::Failure {
                            path: path.display().to_string(),
                            error,
                        },
                    );
                }
            }
        }

        let _ = app_handle.emit(
            "cleanup://progress",
            CleanupProgress::Finished { successes, failures },
        );
    });

    Ok(())
}

fn parse_filter(input: &str) -> Result<DirectoryFilter, String> {
    match input.to_lowercase().as_str() {
        "all" | "todos" => Ok(DirectoryFilter::Todos),
        "images" | "imagenes" => Ok(DirectoryFilter::SoloImagenes),
        "office" => Ok(DirectoryFilter::SoloOffice),
        _ => Err("Filtro no reconocido".to_string()),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            analyze_file,
            analyze_directory,
            analyze_files,
            search_files,
            search_directories,
            remove_metadata,
            edit_office_metadata,
            start_cleanup,
            start_cleanup_files,
            pick_file,
            pick_directory,
            pick_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
