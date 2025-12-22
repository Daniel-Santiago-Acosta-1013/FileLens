//! Operaciones relacionadas con metadata EXIF de imágenes.

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use crate::metadata_editor::utils::generate_temp_filename;

/// Elimina la metadata EXIF de una imagen manteniendo la información visual.
pub fn remove_image_metadata(path: &Path) -> Result<(), String> {
    use image::ImageReader;

    let img = ImageReader::open(path)
        .map_err(|e| format!("No se pudo abrir la imagen: {}", e))?
        .decode()
        .map_err(|e| format!("No se pudo decodificar la imagen: {}", e))?;

    let temp_path = generate_temp_filename(path);

    img.save(&temp_path)
        .map_err(|e| format!("No se pudo guardar la imagen limpia: {}", e))?;

    let metadata_clean = verify_image_metadata_clean(&temp_path)?;

    if !metadata_clean {
        let _ = fs::remove_file(&temp_path);

        return Err(
            "La verificación indicó que la metadata no se eliminó correctamente".to_string(),
        );
    }

    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("No se pudo reemplazar el archivo original: {}", e)
    })?;

    Ok(())
}

/// Comprueba que una imagen carece de campos EXIF residuales tras limpiar su metadata.
pub fn verify_image_metadata_clean(path: &Path) -> Result<bool, String> {
    let file = File::open(path)
        .map_err(|e| format!("No se pudo abrir la imagen limpia para verificación: {}", e))?;
    let mut reader = BufReader::new(file);

    match exif::Reader::new().read_from_container(&mut reader) {
        Ok(exif) => Ok(exif.fields().next().is_none()),
        Err(exif::Error::NotFound(_)) | Err(exif::Error::BlankValue(_)) => Ok(true),
        Err(exif::Error::InvalidFormat(_)) => Ok(true),
        Err(exif::Error::Io(err)) => Err(format!(
            "No se pudo leer metadata EXIF durante la verificación: {}",
            err
        )),
        Err(other) => Err(format!("Error verificando metadata EXIF: {}", other)),
    }
}
