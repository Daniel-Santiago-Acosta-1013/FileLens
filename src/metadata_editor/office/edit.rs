use std::fs;
use std::path::Path;

use crate::metadata_editor::utils::generate_temp_filename;

use super::{app_field_spec, core_field_spec, rewrite_docx, sanitize::apply_xml_updates};

/// Actualiza un campo concreto de la metadata de un documento Office.
pub fn apply_office_metadata_edit(path: &Path, xml_tag: &str, value: &str) -> Result<(), String> {
    enum DocPropsTarget {
        Core,
        App,
    }

    let target = if xml_tag.contains(':') {
        DocPropsTarget::Core
    } else {
        DocPropsTarget::App
    };

    let temp_path = generate_temp_filename(path);

    let changed = rewrite_docx(path, &temp_path, |name, contents| match (name, &target) {
        ("docProps/core.xml", DocPropsTarget::Core) => {
            let updates = [(xml_tag, value); 1];
            apply_xml_updates(contents, &updates, core_field_spec)
        }
        ("docProps/app.xml", DocPropsTarget::App) => {
            let updates = [(xml_tag, value); 1];
            apply_xml_updates(contents, &updates, app_field_spec)
        }
        _ => Ok((contents, false)),
    })?;

    if !changed {
        let _ = fs::remove_file(&temp_path);
        return Err("No se encontró el campo solicitado para modificar".to_string());
    }

    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("No se pudo reemplazar el archivo original: {}", e)
    })?;

    Ok(())
}
