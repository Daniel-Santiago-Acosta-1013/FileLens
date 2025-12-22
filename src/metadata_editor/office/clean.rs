use std::fs;
use std::path::Path;

use crate::metadata_editor::utils::generate_temp_filename;

use super::{
    rewrite_docx, sanitize_app_properties, sanitize_core_properties, sanitize_custom_properties,
    verify::verify_office_metadata_clean,
};

/// Elimina metadata sensible de documentos Office y mantiene el contenido original intacto.
pub fn remove_office_metadata(path: &Path) -> Result<(), String> {
    let temp_path = generate_temp_filename(path);

    let _cleaned_anything = rewrite_docx(path, &temp_path, |name, contents| match name {
        "docProps/core.xml" => {
            sanitize_core_properties(contents).map_err(|e| format!("core.xml: {}", e))
        }
        "docProps/app.xml" => {
            sanitize_app_properties(contents).map_err(|e| format!("app.xml: {}", e))
        }
        "docProps/custom.xml" => Ok(sanitize_custom_properties(contents)),
        _ => Ok((contents, false)),
    })?;

    let metadata_clean = verify_office_metadata_clean(&temp_path)?;

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
