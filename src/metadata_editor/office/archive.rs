use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

/// Reescribe un documento Office aplicando una transformación por archivo.
pub(crate) fn rewrite_docx<F>(
    path: &Path,
    output_path: &Path,
    mut transform: F,
) -> Result<bool, String>
where
    F: FnMut(&str, Vec<u8>) -> Result<(Vec<u8>, bool), String>,
{
    let source_file =
        File::open(path).map_err(|e| format!("No se pudo abrir el archivo: {}", e))?;
    let mut archive = ZipArchive::new(source_file)
        .map_err(|e| format!("No es un documento Office válido: {}", e))?;

    let target_file =
        File::create(output_path).map_err(|e| format!("No se pudo crear archivo limpio: {}", e))?;
    let mut writer = ZipWriter::new(target_file);

    let mut modified_any = false;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Error leyendo archivo del ZIP: {}", e))?;
        let name = file.name().to_string();

        let mut options = FileOptions::<'_, ()>::default().compression_method(file.compression());
        if let Some(mode) = file.unix_mode() {
            options = options.unix_permissions(mode);
        }
        if let Some(time) = file.last_modified() {
            options = options.last_modified_time(time);
        }

        if file.is_dir() {
            writer
                .add_directory(name, options)
                .map_err(|e| format!("Error creando directorio en ZIP: {}", e))?;
            continue;
        }

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| format!("Error leyendo contenido: {}", e))?;

        let (data_to_write, changed) = transform(&name, contents)?;
        if changed {
            modified_any = true;
        }

        writer
            .start_file(name, options)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
        writer
            .write_all(&data_to_write)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
    }

    writer
        .finish()
        .map_err(|e| format!("Error finalizando archivo: {}", e))?;

    Ok(modified_any)
}
