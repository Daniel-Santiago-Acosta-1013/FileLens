//! Detección de tipos MIME mediante inferencia heurística.

use infer::Infer;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct DetectedFileType {
    pub mime: Option<String>,
    pub extension: Option<String>,
}

/// Intenta detectar el tipo MIME del archivo a partir de su contenido.
#[allow(dead_code)]
pub fn mime_type(path: &Path) -> Option<String> {
    detect_file_type(path).mime
}

pub fn detect_file_type(path: &Path) -> DetectedFileType {
    let infer = Infer::new();
    match infer.get_from_path(path) {
        Ok(Some(kind)) => DetectedFileType {
            mime: Some(kind.mime_type().to_string()),
            extension: Some(kind.extension().to_string()),
        },
        _ => DetectedFileType {
            mime: None,
            extension: None,
        },
    }
}
