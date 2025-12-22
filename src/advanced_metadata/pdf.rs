//! Extracción de metadata en PDFs mediante lectura del diccionario Info.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use lopdf::{Document, Object};
use std::path::Path;

pub fn extract_pdf_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata PDF");
    let mut risks = Vec::new();

    let doc = match Document::load(path) {
        Ok(doc) => doc,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer metadata de este PDF",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let info_ref = match doc.trailer.get(b"Info") {
        Ok(info) => info,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se encontró metadata adicional en este PDF",
                EntryLevel::Muted,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let info_dict = match deref_dictionary(&doc, info_ref) {
        Some(dict) => dict,
        None => {
            section.notice = Some(SectionNotice::new(
                "No se encontró metadata adicional en este PDF",
                EntryLevel::Muted,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut has_entries = false;
    has_entries |= push_pdf_entry(&doc, info_dict, b"Title", "Título", false, &mut section, &mut risks);
    has_entries |= push_pdf_entry(&doc, info_dict, b"Author", "Autor", true, &mut section, &mut risks);
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"Subject",
        "Asunto",
        false,
        &mut section,
        &mut risks,
    );
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"Keywords",
        "Palabras clave",
        false,
        &mut section,
        &mut risks,
    );
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"Creator",
        "Creador",
        true,
        &mut section,
        &mut risks,
    );
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"Producer",
        "Productor",
        true,
        &mut section,
        &mut risks,
    );
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"CreationDate",
        "Fecha de creación",
        false,
        &mut section,
        &mut risks,
    );
    has_entries |= push_pdf_entry(
        &doc,
        info_dict,
        b"ModDate",
        "Fecha de modificación",
        false,
        &mut section,
        &mut risks,
    );

    if !has_entries {
        section.notice = Some(SectionNotice::new(
            "No se encontró metadata adicional en este PDF",
            EntryLevel::Muted,
        ));
    } else if !risks.is_empty() {
        section.notice = Some(SectionNotice::new(
            "⚠  Este PDF contiene metadata que puede revelar información del autor y organización",
            EntryLevel::Warning,
        ));
    }

    AdvancedMetadataResult { section, risks }
}

fn deref_dictionary<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a lopdf::Dictionary> {
    match obj {
        Object::Reference(reference) => doc.get_dictionary(*reference).ok(),
        Object::Dictionary(dict) => Some(dict),
        _ => None,
    }
}

fn push_pdf_entry(
    doc: &Document,
    dict: &lopdf::Dictionary,
    key: &[u8],
    label: &str,
    sensitive: bool,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let value = match dict.get(key) {
        Ok(obj) => object_to_string(doc, obj),
        Err(_) => None,
    };

    if let Some(value) = value {
        let level = if sensitive {
            EntryLevel::Warning
        } else {
            EntryLevel::Info
        };
        section.entries.push(ReportEntry::new(label, &value, level));
        if sensitive {
            risks.push(ReportEntry::warning(label, value));
        }
        return true;
    }
    false
}

fn object_to_string(doc: &Document, obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).trim().to_string()),
        Object::Name(name) => Some(String::from_utf8_lossy(name).trim().to_string()),
        Object::Reference(reference) => doc
            .get_object(*reference)
            .ok()
            .and_then(|inner| object_to_string(doc, inner)),
        _ => None,
    }
}
