use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::metadata_editor::constants::{
    APP_SANITIZE_FIELDS, CORE_SANITIZE_FIELDS, CUSTOM_PROPERTIES_EMPTY,
};

use super::xml::{
    FieldSpec, app_field_spec, core_field_spec, element_matches_expected_value,
    element_text_content,
};

/// Comprueba que un documento Office limpio no conserva metadata sensible.
pub fn verify_office_metadata_clean(path: &Path) -> Result<bool, String> {
    let file = File::open(path)
        .map_err(|e| format!("No se pudo abrir archivo limpio para verificación: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("No es un documento Office válido: {}", e))?;

    let core_clean = match archive.by_name("docProps/core.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("No se pudo leer core.xml durante la verificación: {}", e))?;
            is_xml_metadata_clean(&contents, &CORE_SANITIZE_FIELDS, core_field_spec)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a core.xml durante la verificación: {}",
                e
            ));
        }
    };

    let app_clean = match archive.by_name("docProps/app.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("No se pudo leer app.xml durante la verificación: {}", e))?;
            is_xml_metadata_clean(&contents, &APP_SANITIZE_FIELDS, app_field_spec)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a app.xml durante la verificación: {}",
                e
            ));
        }
    };

    let custom_clean = match archive.by_name("docProps/custom.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).map_err(|e| {
                format!("No se pudo leer custom.xml durante la verificación: {}", e)
            })?;
            is_custom_metadata_clean(&contents)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a custom.xml durante la verificación: {}",
                e
            ));
        }
    };

    Ok(core_clean && app_clean && custom_clean)
}

fn is_xml_metadata_clean(
    contents: &[u8],
    expected_values: &[(&str, &str)],
    lookup: fn(&str) -> Option<FieldSpec<'static>>,
) -> Result<bool, String> {
    let root = xmltree::Element::parse(std::io::Cursor::new(contents)).map_err(|e| {
        format!(
            "Error leyendo XML de metadata durante la verificación: {}",
            e
        )
    })?;

    for &(tag, expected) in expected_values {
        if let Some(spec) = lookup(tag)
            && !element_matches_expected_value(&root, spec, expected)
        {
            return Ok(false);
        }
    }

    Ok(true)
}

fn is_custom_metadata_clean(contents: &[u8]) -> Result<bool, String> {
    if contents == CUSTOM_PROPERTIES_EMPTY.as_bytes() {
        return Ok(true);
    }

    let root = xmltree::Element::parse(std::io::Cursor::new(contents))
        .map_err(|e| format!("Error leyendo custom.xml durante la verificación: {}", e))?;

    let has_property_elements = root
        .children
        .iter()
        .any(|node| matches!(node, xmltree::XMLNode::Element(_)));

    if has_property_elements {
        return Ok(false);
    }

    let text = element_text_content(&root);
    Ok(text.is_empty())
}
