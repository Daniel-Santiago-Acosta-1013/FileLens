use std::io::Cursor;

use xmltree::Element;

use crate::metadata_editor::constants::{
    APP_SANITIZE_FIELDS, CORE_SANITIZE_FIELDS, CUSTOM_PROPERTIES_EMPTY,
};

use super::xml::{FieldSpec, app_field_spec, apply_update_to_element, core_field_spec};

/// Normaliza los campos principales de metadata para eliminar rastros de autoría.
pub(crate) fn sanitize_core_properties(contents: Vec<u8>) -> Result<(Vec<u8>, bool), String> {
    apply_xml_updates(contents, &CORE_SANITIZE_FIELDS, core_field_spec)
}

/// Elimina valores específicos de metadata de aplicación (app.xml).
pub(crate) fn sanitize_app_properties(contents: Vec<u8>) -> Result<(Vec<u8>, bool), String> {
    apply_xml_updates(contents, &APP_SANITIZE_FIELDS, app_field_spec)
}

/// Reemplaza el XML de propiedades personalizadas por una plantilla vacía.
pub(crate) fn sanitize_custom_properties(contents: Vec<u8>) -> (Vec<u8>, bool) {
    let sanitized = CUSTOM_PROPERTIES_EMPTY.as_bytes().to_vec();
    let modified = contents != sanitized;
    (sanitized, modified)
}

pub(crate) fn apply_xml_updates(
    contents: Vec<u8>,
    updates: &[(&str, &str)],
    lookup: fn(&str) -> Option<FieldSpec<'static>>,
) -> Result<(Vec<u8>, bool), String> {
    let mut root = Element::parse(Cursor::new(&contents[..]))
        .map_err(|e| format!("Error leyendo XML de metadata: {}", e))?;

    let mut modified = false;
    for &(tag, value) in updates {
        if let Some(spec) = lookup(tag) {
            modified |= apply_update_to_element(&mut root, spec, value);
        }
    }

    if !modified {
        return Ok((contents, false));
    }

    let mut output = Vec::new();
    let mut config = xmltree::EmitterConfig::new();
    config.perform_indent = false;
    config.write_document_declaration = true;
    root.write_with_config(&mut output, config)
        .map_err(|e| format!("Error escribiendo XML sanitizado: {}", e))?;

    Ok((output, true))
}
