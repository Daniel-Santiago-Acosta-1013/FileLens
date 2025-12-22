//! Limpieza masiva de metadata para directorios completos.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use super::removal::remove_all_metadata;

/// Filtros disponibles para seleccionar qué archivos se procesarán.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DirectoryFilter {
    Todos,
    SoloImagenes,
    SoloOffice,
}

impl DirectoryFilter {
    fn matches(self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        let ext = ext.to_lowercase();

        match self {
            DirectoryFilter::Todos => is_supported_image(&ext) || is_supported_office(&ext),
            DirectoryFilter::SoloImagenes => is_supported_image(&ext),
            DirectoryFilter::SoloOffice => is_supported_office(&ext),
        }
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];
const OFFICE_EXTENSIONS: &[&str] = &["docx", "xlsx", "pptx"];
const NO_EXTENSION_LABEL: &str = "sin extensión";

#[derive(Default)]
struct DirectoryAnalysis {
    total_files: usize,
    images_count: usize,
    office_count: usize,
    image_extensions: BTreeSet<String>,
    office_extensions: BTreeSet<String>,
    extension_counts: BTreeMap<String, usize>,
}

impl DirectoryAnalysis {
    fn record_extension(&mut self, ext: Option<&str>) {
        let key = ext
            .map(|e| e.to_string())
            .unwrap_or_else(|| NO_EXTENSION_LABEL.to_string());
        *self.extension_counts.entry(key).or_insert(0) += 1;
    }
}

fn is_supported_image(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

fn is_supported_office(ext: &str) -> bool {
    OFFICE_EXTENSIONS.contains(&ext)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryAnalysisSummary {
    pub total_files: usize,
    pub images_count: usize,
    pub office_count: usize,
    pub extension_counts: Vec<(String, usize)>,
    pub image_extensions: Vec<String>,
    pub office_extensions: Vec<String>,
}

impl DirectoryAnalysisSummary {
    pub fn supported_total(&self) -> usize {
        self.images_count + self.office_count
    }
}

impl From<&DirectoryAnalysis> for DirectoryAnalysisSummary {
    fn from(analysis: &DirectoryAnalysis) -> Self {
        let mut items: Vec<_> = analysis.extension_counts.iter().collect();
        items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

        Self {
            total_files: analysis.total_files,
            images_count: analysis.images_count,
            office_count: analysis.office_count,
            extension_counts: items
                .into_iter()
                .map(|(ext, count)| (ext.clone(), *count))
                .collect(),
            image_extensions: analysis.image_extensions.iter().cloned().collect(),
            office_extensions: analysis.office_extensions.iter().cloned().collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CleanupEvent {
    Started { total: usize },
    Processing { index: usize, total: usize, path: PathBuf },
    Success { path: PathBuf },
    Failure { path: PathBuf, error: String },
    Finished { successes: usize, failures: usize },
}

pub fn collect_candidate_files(
    root: &Path,
    recursive: bool,
    filter: DirectoryFilter,
) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err("La ruta proporcionada no es un directorio".to_string());
    }

    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut files = Vec::new();

    while let Some(dir) = queue.pop_front() {
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| format!("Entrada inválida en {}: {}", dir.display(), e))?;
            let path = entry.path();

            if path.is_dir() {
                if recursive {
                    queue.push_back(path);
                }
                continue;
            }

            if filter.matches(&path) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub fn analyze_directory(path: &Path, recursive: bool) -> Result<DirectoryAnalysisSummary, String> {
    let analysis = analyze_directory_content(path, recursive)?;
    Ok(DirectoryAnalysisSummary::from(&analysis))
}

fn analyze_directory_content(root: &Path, recursive: bool) -> Result<DirectoryAnalysis, String> {
    if !root.is_dir() {
        return Err("La ruta proporcionada no es un directorio".to_string());
    }

    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut analysis = DirectoryAnalysis::default();

    while let Some(dir) = queue.pop_front() {
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| format!("Entrada inválida en {}: {}", dir.display(), e))?;
            let path = entry.path();

            if path.is_dir() {
                if recursive {
                    queue.push_back(path);
                }
                continue;
            }

            analysis.total_files += 1;

            let ext_owned = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            let ext = ext_owned.as_deref();

            if let Some(ext) = ext {
                if is_supported_image(ext) {
                    analysis.images_count += 1;
                    analysis.image_extensions.insert(ext.to_string());
                }
                if is_supported_office(ext) {
                    analysis.office_count += 1;
                    analysis.office_extensions.insert(ext.to_string());
                }
            }

            analysis.record_extension(ext);
        }
    }

    Ok(analysis)
}

pub fn run_cleanup_with_sender(
    files: Vec<PathBuf>,
    sender: Sender<CleanupEvent>,
) -> Result<(), String> {
    let total = files.len();
    let _ = sender.send(CleanupEvent::Started { total });

    let mut successes = 0_usize;
    let mut failures = 0_usize;

    for (index, path) in files.into_iter().enumerate() {
        let _ = sender.send(CleanupEvent::Processing {
            index: index + 1,
            total,
            path: path.clone(),
        });

        match remove_all_metadata(&path) {
            Ok(()) => {
                successes += 1;
                let _ = sender.send(CleanupEvent::Success { path });
            }
            Err(error) => {
                failures += 1;
                let _ = sender.send(CleanupEvent::Failure { path, error });
            }
        }
    }

    let _ = sender.send(CleanupEvent::Finished { successes, failures });
    Ok(())
}
