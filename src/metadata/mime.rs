//! Detección de tipos MIME mediante inferencia heurística.

use infer::Infer;
use std::path::Path;

/// Intenta detectar el tipo MIME del archivo a partir de su contenido.
pub fn mime_type(path: &Path) -> Option<String> {
    let infer = Infer::new();
    infer
        .get_from_path(path)
        .ok()
        .flatten()
        .map(|kind| kind.mime_type().to_string())
}
