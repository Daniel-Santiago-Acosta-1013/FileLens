//! Utilidades compartidas para generar rutas temporales.

use std::path::{Path, PathBuf};

/// Crea un nombre de archivo temporal estable en el mismo directorio que `path`.
pub fn generate_temp_filename(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();

    // Usar timestamp para evitar colisiones entre ejecuciones consecutivas.
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    parent.join(format!(".{}_temp_{}.{}", stem, timestamp, extension))
}
