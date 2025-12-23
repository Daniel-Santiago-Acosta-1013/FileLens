//! Extracción de metadata en PDFs mediante lectura del diccionario Info.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use lopdf::{Document, Object, ObjectId};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::xmp::parse_xmp_metadata;

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

    let mut has_entries = false;

    has_entries |= push_simple_entry(
        &mut section,
        "Versión PDF",
        doc.version.clone(),
        EntryLevel::Info,
    );

    let linearized = is_pdf_linearized(path);
    has_entries |= push_simple_entry(
        &mut section,
        "Linealizado",
        if linearized { "Sí" } else { "No" },
        EntryLevel::Info,
    );
    has_entries |= push_simple_entry(
        &mut section,
        "Optimizado",
        if linearized { "Sí" } else { "No/Desconocido" },
        EntryLevel::Info,
    );

    if let Some(updates) = count_incremental_updates(path) {
        let value = if updates > 0 {
            format!("{updates}")
        } else {
            "0".to_string()
        };
        has_entries |= push_simple_entry(
            &mut section,
            "Actualizaciones incrementales",
            value,
            EntryLevel::Info,
        );
    }

    if let Some(ids) = pdf_trailer_ids(&doc) {
        has_entries |= push_simple_entry(&mut section, "Trailer IDs", ids, EntryLevel::Info);
    }
    if let Ok(info_ref) = doc.trailer.get(b"Info")
        && let Some(info_dict) = deref_dictionary(&doc, info_ref)
    {
        has_entries |= push_pdf_entry(
            &doc,
            info_dict,
            b"Title",
            "Título",
            false,
            &mut section,
            &mut risks,
        );
        has_entries |= push_pdf_entry(
            &doc,
            info_dict,
            b"Author",
            "Autor",
            true,
            &mut section,
            &mut risks,
        );
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

        if has_custom_info_fields(info_dict) {
            has_entries |= push_simple_entry(
                &mut section,
                "Metadata personalizada",
                "Sí",
                EntryLevel::Warning,
            );
        }
    }

    if let Some(xmp_packet) = extract_pdf_xmp(&doc) {
        let _ = push_simple_entry(&mut section, "XMP stream", "Sí", EntryLevel::Info);
        let entries_before = section.entries.len();
        let mut xmp_added = false;
        if let Some(xmp) = parse_xmp_metadata(&xmp_packet) {
            for entry in xmp.entries {
                section.entries.push(entry);
            }
            if !xmp.risks.is_empty() {
                risks.extend(xmp.risks);
            }
            xmp_added = section.entries.len() > entries_before;
        }
        if !xmp_added {
            section
                .entries
                .push(ReportEntry::warning("XMP", "Detectado"));
            risks.push(ReportEntry::warning(
                "XMP embebido",
                "Puede contener metadata adicional",
            ));
        }
        has_entries = true;
    } else {
        has_entries |= push_simple_entry(&mut section, "XMP stream", "No", EntryLevel::Info);
    }

    has_entries |= append_pdf_security(&doc, &mut section, &mut risks);
    has_entries |= append_pdf_structure(&doc, &mut section, &mut risks);

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

fn push_simple_entry(
    section: &mut ReportSection,
    label: &str,
    value: impl Into<String>,
    level: EntryLevel,
) -> bool {
    let value = value.into();
    if value.trim().is_empty() {
        return false;
    }
    section.entries.push(ReportEntry::new(label, value, level));
    true
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

fn object_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(value) => Some(*value as f64),
        Object::Integer(value) => Some(*value as f64),
        _ => None,
    }
}

fn is_pdf_linearized(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buffer = [0_u8; 2048];
    let bytes = match file.read(&mut buffer) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let text = String::from_utf8_lossy(&buffer[..bytes]);
    text.contains("/Linearized")
}

fn count_incremental_updates(path: &Path) -> Option<usize> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0_u8; 8192];
    let mut count = 0;
    let mut tail = Vec::new();
    loop {
        let bytes = file.read(&mut buffer).ok()?;
        if bytes == 0 {
            break;
        }
        let mut chunk = tail.clone();
        chunk.extend_from_slice(&buffer[..bytes]);
        count += count_subslice(&chunk, b"startxref");
        tail = chunk[chunk.len().saturating_sub(16)..].to_vec();
    }
    Some(count.saturating_sub(1))
}

fn count_subslice(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn pdf_trailer_ids(doc: &Document) -> Option<String> {
    let ids = doc.trailer.get(b"ID").ok()?;
    let array = match ids {
        Object::Array(values) => values,
        _ => return None,
    };
    let mut strings = Vec::new();
    for value in array {
        if let Some(text) = object_to_string(doc, value) {
            strings.push(text);
        }
    }
    if strings.is_empty() {
        None
    } else {
        Some(strings.join(" / "))
    }
}

fn has_custom_info_fields(info: &lopdf::Dictionary) -> bool {
    let standard: [&[u8]; 9] = [
        b"Title",
        b"Author",
        b"Subject",
        b"Keywords",
        b"Creator",
        b"Producer",
        b"CreationDate",
        b"ModDate",
        b"Trapped",
    ];
    for (key, _) in info.iter() {
        if !standard.iter().any(|item| *item == key.as_slice()) {
            return true;
        }
    }
    false
}

fn append_pdf_security(
    doc: &Document,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let mut has_entries = false;
    let encrypted = doc.is_encrypted();
    let level = if encrypted {
        EntryLevel::Warning
    } else {
        EntryLevel::Info
    };
    has_entries |= push_simple_entry(
        section,
        "Encriptado",
        if encrypted { "Sí" } else { "No" },
        level,
    );

    if encrypted {
        if let Ok(dict) = doc.get_encrypted() {
            if let Ok(filter) = dict.get(b"Filter").and_then(Object::as_name) {
                has_entries |= push_simple_entry(
                    section,
                    "Algoritmo",
                    String::from_utf8_lossy(filter).to_string(),
                    EntryLevel::Warning,
                );
            }
            if let Ok(version) = dict.get(b"V").and_then(Object::as_i64) {
                has_entries |= push_simple_entry(
                    section,
                    "Versión de seguridad",
                    version.to_string(),
                    EntryLevel::Warning,
                );
            }
            if let Ok(revision) = dict.get(b"R").and_then(Object::as_i64) {
                has_entries |= push_simple_entry(
                    section,
                    "Revisión de seguridad",
                    revision.to_string(),
                    EntryLevel::Warning,
                );
            }
            if let Ok(length) = dict.get(b"Length").and_then(Object::as_i64) {
                has_entries |= push_simple_entry(
                    section,
                    "Longitud de clave",
                    format!("{length} bits"),
                    EntryLevel::Warning,
                );
            }
            if let Ok(perms) = dict.get(b"P").and_then(Object::as_i64) {
                has_entries |= push_simple_entry(
                    section,
                    "Permisos",
                    format_pdf_permissions(perms),
                    EntryLevel::Warning,
                );
            }
        }
    }

    let (sig_count, cert_count) = count_pdf_signatures(doc);
    has_entries |= push_simple_entry(
        section,
        "Firmas digitales",
        sig_count.to_string(),
        EntryLevel::Info,
    );
    if cert_count > 0 {
        has_entries |= push_simple_entry(
            section,
            "Certificados",
            cert_count.to_string(),
            EntryLevel::Info,
        );
    }

    if encrypted {
        risks.push(ReportEntry::warning(
            "PDF encriptado",
            "Puede contener permisos restringidos o contenido protegido",
        ));
    }

    has_entries
}

fn count_pdf_signatures(doc: &Document) -> (usize, usize) {
    let mut signatures = 0;
    let mut certs = 0;
    for (_, obj) in doc.objects.iter() {
        let dict = match obj {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        let is_sig = matches!(
            dict.get(b"Type").and_then(Object::as_name),
            Ok(b"Sig")
        ) || matches!(
            dict.get(b"FT").and_then(Object::as_name),
            Ok(b"Sig")
        );
        if is_sig {
            signatures += 1;
            if dict.get(b"Cert").is_ok() {
                certs += 1;
            }
        }
    }
    (signatures, certs)
}

fn format_pdf_permissions(perms: i64) -> String {
    let bits = perms as u32;
    let allow_print = bits & 0b0000_0100 != 0;
    let allow_modify = bits & 0b0000_1000 != 0;
    let allow_copy = bits & 0b0001_0000 != 0;
    let allow_annot = bits & 0b0010_0000 != 0;
    let allow_fill = bits & 0b0001_0000_0000 != 0;
    let allow_access = bits & 0b0010_0000_0000 != 0;
    let allow_assemble = bits & 0b0100_0000_0000 != 0;
    let allow_print_high = bits & 0b1000_0000_0000 != 0;
    format!(
        "Imprimir: {}, Modificar: {}, Copiar: {}, Anotar: {}, Rellenar: {}, Accesibilidad: {}, Ensamblar: {}, Imprimir alta: {}",
        yes_no(allow_print),
        yes_no(allow_modify),
        yes_no(allow_copy),
        yes_no(allow_annot),
        yes_no(allow_fill),
        yes_no(allow_access),
        yes_no(allow_assemble),
        yes_no(allow_print_high)
    )
}

fn yes_no(value: bool) -> &'static str {
    if value { "Sí" } else { "No" }
}

fn append_pdf_structure(
    doc: &Document,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    const PAGE_LIMIT: usize = 10;
    const FONT_LIMIT: usize = 25;
    const IMAGE_LIMIT: usize = 25;

    let mut has_entries = false;
    let pages = doc.get_pages();
    has_entries |= push_simple_entry(
        section,
        "Recuento de páginas",
        pages.len().to_string(),
        EntryLevel::Info,
    );

    if let Ok(catalog) = doc.catalog() {
        let tagged = catalog
            .get(b"MarkInfo")
            .and_then(Object::as_dict)
            .and_then(|dict| dict.get(b"Marked"))
            .and_then(Object::as_bool)
            .unwrap_or(false);
        has_entries |= push_simple_entry(
            section,
            "Tagged",
            if tagged { "Sí" } else { "No" },
            EntryLevel::Info,
        );

        let has_struct = catalog.get(b"StructTreeRoot").is_ok();
        has_entries |= push_simple_entry(
            section,
            "Marked content",
            if has_struct { "Sí" } else { "No" },
            EntryLevel::Info,
        );

        if let Ok(outlines) = catalog.get(b"Outlines") {
            let outline_count = count_outlines(doc, outlines);
            has_entries |= push_simple_entry(
                section,
                "Outlines",
                outline_count.to_string(),
                EntryLevel::Info,
            );
        }

        if let Ok(acroform) = catalog.get(b"AcroForm") {
            has_entries |= push_simple_entry(section, "AcroForm", "Sí", EntryLevel::Info);
            if let Ok(dict) = acroform.as_dict() {
                let has_xfa = dict.get(b"XFA").is_ok();
                has_entries |= push_simple_entry(
                    section,
                    "XFA",
                    if has_xfa { "Sí" } else { "No" },
                    EntryLevel::Info,
                );
            }
        } else {
            has_entries |= push_simple_entry(section, "AcroForm", "No", EntryLevel::Info);
        }

        if let Ok(names) = catalog.get(b"Names") {
            let attachments = count_embedded_files(doc, names);
            has_entries |= push_simple_entry(
                section,
                "Adjuntos",
                attachments.to_string(),
                EntryLevel::Info,
            );
        }
    }

    let action_counts = scan_pdf_actions(doc);
    has_entries |= push_simple_entry(
        section,
        "JavaScript",
        action_counts.javascript.to_string(),
        EntryLevel::Info,
    );
    has_entries |= push_simple_entry(
        section,
        "Acciones Launch/URI",
        (action_counts.launch + action_counts.uri).to_string(),
        EntryLevel::Info,
    );
    has_entries |= push_simple_entry(
        section,
        "Anotaciones",
        action_counts.annotations.to_string(),
        EntryLevel::Info,
    );

    let mut suspicious = Vec::new();
    if action_counts.javascript > 0 {
        suspicious.push("JavaScript".to_string());
    }
    if action_counts.launch > 0 {
        suspicious.push("Launch".to_string());
    }
    if action_counts.uri > 0 {
        suspicious.push("URI".to_string());
    }
    if action_counts.embedded_files > 0 {
        suspicious.push("EmbeddedFile".to_string());
    }
    if action_counts.rich_media > 0 {
        suspicious.push("RichMedia".to_string());
    }
    if !suspicious.is_empty() {
        has_entries |= push_simple_entry(
            section,
            "Objetos sospechosos",
            suspicious.join(", "),
            EntryLevel::Warning,
        );
        risks.push(ReportEntry::warning(
            "Objetos sospechosos",
            suspicious.join(", "),
        ));
    }

    for (index, (page_num, page_id)) in pages.iter().take(PAGE_LIMIT).enumerate() {
        if let Ok(dict) = doc.get_dictionary(*page_id) {
            if let Some(size) = pdf_page_box(dict, b"MediaBox").or_else(|| pdf_page_box(dict, b"CropBox")) {
                has_entries |= push_simple_entry(
                    section,
                    &format!("Página {} · Tamaño", page_num),
                    size,
                    EntryLevel::Info,
                );
            }
            if let Ok(rotation) = dict.get(b"Rotate").and_then(Object::as_i64) {
                has_entries |= push_simple_entry(
                    section,
                    &format!("Página {} · Rotación", page_num),
                    rotation.to_string(),
                    EntryLevel::Info,
                );
            }
            let (fonts, images, xobjects) = count_page_resources(doc, dict);
            has_entries |= push_simple_entry(
                section,
                &format!("Página {} · Recursos", page_num),
                format!("fonts:{fonts}, images:{images}, xobjects:{xobjects}"),
                EntryLevel::Info,
            );
        }
        if index + 1 == PAGE_LIMIT && pages.len() > PAGE_LIMIT {
            has_entries |= push_simple_entry(
                section,
                "Páginas adicionales omitidas",
                (pages.len() - PAGE_LIMIT).to_string(),
                EntryLevel::Muted,
            );
        }
    }

    let fonts = collect_fonts(doc);
    if !fonts.is_empty() {
        has_entries |= push_simple_entry(
            section,
            "Fuentes",
            fonts.len().to_string(),
            EntryLevel::Info,
        );
        for font in fonts.iter().take(FONT_LIMIT) {
            has_entries |= push_simple_entry(
                section,
                &format!("Fuente · {}", font.name),
                font.summary(),
                EntryLevel::Info,
            );
        }
        if fonts.len() > FONT_LIMIT {
            has_entries |= push_simple_entry(
                section,
                "Fuentes omitidas",
                (fonts.len() - FONT_LIMIT).to_string(),
                EntryLevel::Muted,
            );
        }
    }

    let images = collect_images(doc, &pages);
    if !images.is_empty() {
        has_entries |= push_simple_entry(
            section,
            "Imágenes",
            images.len().to_string(),
            EntryLevel::Info,
        );
        for image in images.iter().take(IMAGE_LIMIT) {
            has_entries |= push_simple_entry(
                section,
                &format!("Imagen · Página {}", image.page),
                image.summary(),
                EntryLevel::Info,
            );
        }
        if images.len() > IMAGE_LIMIT {
            has_entries |= push_simple_entry(
                section,
                "Imágenes omitidas",
                (images.len() - IMAGE_LIMIT).to_string(),
                EntryLevel::Muted,
            );
        }
    }

    has_entries
}

struct ActionCounts {
    javascript: usize,
    launch: usize,
    uri: usize,
    annotations: usize,
    embedded_files: usize,
    rich_media: usize,
}

fn scan_pdf_actions(doc: &Document) -> ActionCounts {
    let mut counts = ActionCounts {
        javascript: 0,
        launch: 0,
        uri: 0,
        annotations: 0,
        embedded_files: 0,
        rich_media: 0,
    };
    for (_, obj) in doc.objects.iter() {
        let dict = match obj {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        if matches!(dict.get(b"Type").and_then(Object::as_name), Ok(b"Annot")) {
            counts.annotations += 1;
        }
        if matches!(dict.get(b"Type").and_then(Object::as_name), Ok(b"EmbeddedFile")) {
            counts.embedded_files += 1;
        }
        if matches!(dict.get(b"Type").and_then(Object::as_name), Ok(b"RichMedia")) {
            counts.rich_media += 1;
        }
        if let Ok(action) = dict.get(b"S").and_then(Object::as_name) {
            match action {
                b"JavaScript" => counts.javascript += 1,
                b"Launch" => counts.launch += 1,
                b"URI" => counts.uri += 1,
                _ => {}
            }
        }
    }
    counts
}

fn pdf_page_box(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    let array = dict.get(key).ok()?.as_array().ok()?;
    if array.len() < 4 {
        return None;
    }
    let x0 = object_to_f64(&array[0])?;
    let y0 = object_to_f64(&array[1])?;
    let x1 = object_to_f64(&array[2])?;
    let y1 = object_to_f64(&array[3])?;
    let width = (x1 - x0).abs();
    let height = (y1 - y0).abs();
    Some(format!("{width:.2} x {height:.2}"))
}

fn count_page_resources(doc: &Document, page: &lopdf::Dictionary) -> (usize, usize, usize) {
    let mut fonts = 0;
    let mut xobjects = 0;
    let mut images = 0;
    if let Ok(resources) = page.get(b"Resources") {
        if let Ok(dict) = resources.as_dict() {
            if let Ok(font_dict) = dict.get(b"Font").and_then(Object::as_dict) {
                fonts = font_dict.len();
            }
            if let Ok(xobj_dict) = dict.get(b"XObject").and_then(Object::as_dict) {
                xobjects = xobj_dict.len();
                for (_, obj) in xobj_dict.iter() {
                    if let Ok(obj_ref) = obj.as_reference() {
                        if let Ok(stream) = doc.get_object(obj_ref).and_then(Object::as_stream) {
                            if stream
                                .dict
                                .get(b"Subtype")
                                .and_then(Object::as_name)
                                .map(|name| name == b"Image")
                                .unwrap_or(false)
                            {
                                images += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    (fonts, images, xobjects)
}

fn count_embedded_files(doc: &Document, names: &Object) -> usize {
    match names {
        Object::Dictionary(dict) => {
            if let Ok(embedded) = dict.get(b"EmbeddedFiles") {
                return count_name_tree(doc, embedded);
            }
            0
        }
        Object::Reference(reference) => doc
            .get_object(*reference)
            .ok()
            .map(|obj| count_embedded_files(doc, obj))
            .unwrap_or(0),
        _ => 0,
    }
}

fn count_name_tree(doc: &Document, obj: &Object) -> usize {
    match obj {
        Object::Dictionary(dict) => {
            let mut count = 0;
            if let Ok(Object::Array(names)) = dict.get(b"Names") {
                count += names.len() / 2;
            }
            if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
                for kid in kids {
                    count += count_name_tree(doc, kid);
                }
            }
            count
        }
        Object::Reference(reference) => doc
            .get_object(*reference)
            .ok()
            .map(|obj| count_name_tree(doc, obj))
            .unwrap_or(0),
        _ => 0,
    }
}

fn count_outlines(doc: &Document, obj: &Object) -> usize {
    let dict = match obj {
        Object::Reference(reference) => doc.get_dictionary(*reference).ok(),
        Object::Dictionary(dict) => Some(dict),
        _ => None,
    };
    let Some(dict) = dict else { return 0 };
    let first = dict.get(b"First").ok();
    count_outline_chain(doc, first)
}

fn count_outline_chain(doc: &Document, first: Option<&Object>) -> usize {
    let mut count = 0;
    let mut current = first.and_then(|obj| obj.as_reference().ok());
    while let Some(id) = current {
        if let Ok(dict) = doc.get_dictionary(id) {
            count += 1;
            if let Ok(first_child) = dict.get(b"First") {
                count += count_outline_chain(doc, Some(first_child));
            }
            current = dict.get(b"Next").and_then(Object::as_reference).ok();
        } else {
            break;
        }
    }
    count
}

#[derive(Clone)]
struct FontInfo {
    name: String,
    subtype: String,
    encoding: Option<String>,
    embedded: bool,
    subset: bool,
    unicode: bool,
    object_id: ObjectId,
}

impl FontInfo {
    fn summary(&self) -> String {
        format!(
            "Tipo: {}, Encoding: {}, Embebido: {}, Subconjunto: {}, Unicode: {}, Obj: {} {}",
            self.subtype,
            self.encoding.clone().unwrap_or_else(|| "N/D".to_string()),
            yes_no(self.embedded),
            yes_no(self.subset),
            yes_no(self.unicode),
            self.object_id.0,
            self.object_id.1
        )
    }
}

fn collect_fonts(doc: &Document) -> Vec<FontInfo> {
    let mut fonts = Vec::new();
    let mut seen = HashSet::new();
    for (id, obj) in doc.objects.iter() {
        let dict = match obj {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        let subtype = dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .ok()
            .map(|name| String::from_utf8_lossy(name).to_string());
        if subtype.is_none() {
            continue;
        }
        let name = dict
            .get(b"BaseFont")
            .ok()
            .and_then(|obj| object_to_string(doc, obj))
            .unwrap_or_else(|| "Desconocido".to_string());
        let subset = name.contains('+');
        if !seen.insert(name.clone()) {
            continue;
        }
        let encoding = dict
            .get(b"Encoding")
            .ok()
            .and_then(|obj| object_to_string(doc, obj));
        let unicode = dict.get(b"ToUnicode").is_ok();
        let embedded = dict
            .get(b"FontDescriptor")
            .and_then(Object::as_dict)
            .map(|desc| {
                desc.get(b"FontFile").is_ok()
                    || desc.get(b"FontFile2").is_ok()
                    || desc.get(b"FontFile3").is_ok()
            })
            .unwrap_or(false);

        fonts.push(FontInfo {
            name,
            subtype: subtype.unwrap_or_else(|| "Desconocido".to_string()),
            encoding,
            embedded,
            subset,
            unicode,
            object_id: *id,
        });
    }
    fonts
}

struct ImageInfo {
    page: u32,
    width: i64,
    height: i64,
    color_space: Option<String>,
    components: Option<u8>,
    bits_per_component: Option<i64>,
    filters: Option<String>,
    interpolate: bool,
    stream_len: usize,
    object_id: ObjectId,
}

impl ImageInfo {
    fn summary(&self) -> String {
        let ratio = self
            .raw_size()
            .and_then(|raw| if raw > 0 { Some(self.stream_len as f64 / raw as f64) } else { None })
            .map(|value| format!("{value:.2}"));
        format!(
            "{}x{} | CS:{} | BPC:{} | Filt:{} | Interp:{} | Stream:{} bytes | Ratio:{} | Obj:{} {}",
            self.width,
            self.height,
            self.color_space.clone().unwrap_or_else(|| "N/D".to_string()),
            self.bits_per_component.map(|v| v.to_string()).unwrap_or_else(|| "N/D".to_string()),
            self.filters.clone().unwrap_or_else(|| "N/D".to_string()),
            yes_no(self.interpolate),
            self.stream_len,
            ratio.unwrap_or_else(|| "N/D".to_string()),
            self.object_id.0,
            self.object_id.1
        )
    }

    fn raw_size(&self) -> Option<u64> {
        let components = self.components? as u64;
        let bpc = self.bits_per_component? as u64;
        let bits = (self.width.max(0) as u64)
            .saturating_mul(self.height.max(0) as u64)
            .saturating_mul(components)
            .saturating_mul(bpc);
        Some(bits / 8)
    }
}

fn collect_images(doc: &Document, pages: &BTreeMap<u32, ObjectId>) -> Vec<ImageInfo> {
    let mut images = Vec::new();
    for (page_num, page_id) in pages {
        if let Ok(page_images) = doc.get_page_images(*page_id) {
            for image in page_images {
                let color_space = image.color_space.clone();
                let components = color_space.as_deref().and_then(color_space_components);
                let filters = image.filters.as_ref().map(|f| f.join(", "));
                let interpolate = image
                    .origin_dict
                    .get(b"Interpolate")
                    .and_then(Object::as_bool)
                    .unwrap_or(false);
                images.push(ImageInfo {
                    page: *page_num,
                    width: image.width,
                    height: image.height,
                    color_space,
                    components,
                    bits_per_component: image.bits_per_component,
                    filters,
                    interpolate,
                    stream_len: image.content.len(),
                    object_id: image.id,
                });
            }
        }
    }
    images
}

fn color_space_components(space: &str) -> Option<u8> {
    match space {
        "DeviceRGB" => Some(3),
        "DeviceGray" => Some(1),
        "DeviceCMYK" => Some(4),
        _ => None,
    }
}

fn extract_pdf_xmp(doc: &Document) -> Option<String> {
    let catalog = doc.catalog().ok()?;
    let metadata_obj = catalog.get(b"Metadata").ok()?;
    let stream = deref_stream(doc, metadata_obj)?;
    let content = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    if content.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&content).to_string())
}

fn deref_stream<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a lopdf::Stream> {
    match obj {
        Object::Reference(reference) => doc
            .get_object(*reference)
            .ok()
            .and_then(|inner| inner.as_stream().ok()),
        Object::Stream(stream) => Some(stream),
        _ => None,
    }
}
