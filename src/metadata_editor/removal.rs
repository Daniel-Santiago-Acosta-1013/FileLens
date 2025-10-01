//! Lógica de eliminación de metadata según el tipo de archivo.

use console::style;
use std::path::Path;

use super::image::remove_image_metadata;
use super::office::remove_office_metadata;

/// Despacha la limpieza de metadata en función de la extensión del archivo.
pub fn remove_all_metadata(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "tiff" | "tif" => remove_image_metadata(path),
        "docx" | "xlsx" | "pptx" => remove_office_metadata(path),
        "pdf" => {
            println!(
                "\n{}",
                style("│ La eliminación de metadata en PDF está limitada debido a la estructura del formato.")
                    .yellow()
            );
            Err("Formato PDF no soportado completamente para eliminación".to_string())
        }
        _ => Err(format!(
            "Formato .{} no soportado para eliminación de metadata",
            extension
        )),
    }
}
