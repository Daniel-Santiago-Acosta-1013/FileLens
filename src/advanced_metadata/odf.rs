//! Extraccion de metadata para documentos ODF (ODT/ODS/ODP).

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use xmltree::{Element, XMLNode};

const META_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:meta:1.0";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const TABLE_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:table:1.0";
const TEXT_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";
const DRAW_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0";
const PRESENTATION_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:presentation:1.0";

const CONTENT_LIMIT: u64 = 8 * 1024 * 1024;
const META_LIMIT: u64 = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OdfKind {
    Text,
    Spreadsheet,
    Presentation,
    Unknown,
}

pub fn extract_odf_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata ODF");
    let mut risks = Vec::new();

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer metadata ODF",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut header = [0_u8; 4];
    let is_zip = file
        .read(&mut header)
        .ok()
        .map(|read| read >= 2 && &header[..2] == b"PK")
        .unwrap_or(false);
    let _ = file.seek(SeekFrom::Start(0));

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(_) => {
            section.entries.push(ReportEntry::info(
                "Es ZIP",
                if is_zip { "Si" } else { "No" },
            ));
            section.notice = Some(SectionNotice::new(
                "No se pudo leer el paquete ODF",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut has_entries = true;
    section.entries.push(ReportEntry::info(
        "Es ZIP",
        if is_zip { "Si" } else { "No" },
    ));
    section
        .entries
        .push(ReportEntry::info("Entradas totales", archive.len().to_string()));
    let mut odf_kind = OdfKind::Unknown;
    if let Some(mimetype) = read_zip_string(&mut archive, "mimetype", 4096) {
        let trimmed = mimetype.trim().to_string();
        if !trimmed.is_empty() {
            section
                .entries
                .push(ReportEntry::info("Mimetype interno", trimmed.clone()));
            odf_kind = kind_from_mimetype(&trimmed);
            if odf_kind != OdfKind::Unknown {
                section.entries.push(ReportEntry::info(
                    "Tipo ODF",
                    kind_label(odf_kind),
                ));
            }
        }
    }

    let encrypted = manifest_is_encrypted(&mut archive);
    section.entries.push(ReportEntry::info(
        "Cifrado ODF",
        if encrypted { "Si" } else { "No" },
    ));

    if let Some(meta_xml) = read_zip_string(&mut archive, "meta.xml", META_LIMIT) {
        if let Some(root) = parse_xml(&meta_xml) {
            has_entries |= extract_meta_properties(&root, &mut section, &mut risks);
            has_entries |= extract_meta_stats(&root, &mut section);
        }
    }

    if let Some(content_xml) = read_zip_string(&mut archive, "content.xml", CONTENT_LIMIT) {
        if let Some(root) = parse_xml(&content_xml) {
            has_entries |= extract_odf_content(odf_kind, &root, &mut section);
        }
    }

    if !has_entries {
        section.notice = Some(SectionNotice::new(
            "No se encontro metadata adicional en este ODF",
            EntryLevel::Muted,
        ));
    } else if !risks.is_empty() {
        section.notice = Some(SectionNotice::new(
            "Este documento contiene metadata sensible",
            EntryLevel::Warning,
        ));
    }

    AdvancedMetadataResult { section, risks }
}

fn kind_from_mimetype(mimetype: &str) -> OdfKind {
    match mimetype {
        "application/vnd.oasis.opendocument.text" => OdfKind::Text,
        "application/vnd.oasis.opendocument.spreadsheet" => OdfKind::Spreadsheet,
        "application/vnd.oasis.opendocument.presentation" => OdfKind::Presentation,
        _ => OdfKind::Unknown,
    }
}

fn kind_label(kind: OdfKind) -> &'static str {
    match kind {
        OdfKind::Text => "ODT",
        OdfKind::Spreadsheet => "ODS",
        OdfKind::Presentation => "ODP",
        OdfKind::Unknown => "Desconocido",
    }
}

fn read_zip_string(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    limit: u64,
) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    if file.size() > limit {
        return None;
    }
    let mut buffer = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).to_string())
}

fn manifest_is_encrypted(archive: &mut zip::ZipArchive<File>) -> bool {
    let Some(manifest) = read_zip_string(archive, "META-INF/manifest.xml", META_LIMIT) else {
        return false;
    };
    let lowered = manifest.to_lowercase();
    lowered.contains("encryption-data")
}

fn parse_xml(contents: &str) -> Option<Element> {
    Element::parse(contents.as_bytes()).ok()
}

fn extract_meta_properties(
    root: &Element,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let mut has_entries = false;

    let title = first_text_value(root, "title", Some(DC_NS));
    if let Some(value) = title {
        section.entries.push(ReportEntry::info("Titulo", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "subject", Some(DC_NS)) {
        section.entries.push(ReportEntry::info("Asunto", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "description", Some(DC_NS)) {
        section
            .entries
            .push(ReportEntry::info("Descripcion", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "creator", Some(DC_NS)) {
        section
            .entries
            .push(ReportEntry::warning("Creador", &value));
        risks.push(ReportEntry::warning("Creador", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "initial-creator", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::warning("Creador inicial", &value));
        risks.push(ReportEntry::warning("Creador inicial", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "creation-date", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Fecha de creacion", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "date", Some(META_NS))
        .or_else(|| first_text_value(root, "date", Some(DC_NS)))
    {
        section
            .entries
            .push(ReportEntry::info("Fecha de modificacion", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "generator", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Generador", value));
        has_entries = true;
    }

    let keywords = collect_text_values(root, "keyword", Some(META_NS));
    if !keywords.is_empty() {
        section
            .entries
            .push(ReportEntry::info("Keywords", keywords.join(", ")));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "language", Some(DC_NS)) {
        section
            .entries
            .push(ReportEntry::info("Idioma", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "editing-cycles", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Ciclos de edicion", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "editing-duration", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Duracion de edicion", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "printed-by", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Impreso por", value));
        has_entries = true;
    }

    if let Some(value) = first_text_value(root, "print-date", Some(META_NS)) {
        section
            .entries
            .push(ReportEntry::info("Fecha de impresion", value));
        has_entries = true;
    }

    has_entries
}

fn extract_meta_stats(root: &Element, section: &mut ReportSection) -> bool {
    let Some(stat) = find_element(root, "document-statistic", Some(META_NS)) else {
        return false;
    };
    let mut has_entries = false;
    let fields = [
        ("page-count", "Paginas"),
        ("word-count", "Palabras"),
        ("character-count", "Caracteres"),
        ("paragraph-count", "Parrafos"),
        ("table-count", "Tablas"),
        ("image-count", "Imagenes"),
        ("object-count", "Objetos"),
    ];
    for (key, label) in fields {
        if let Some(value) = get_attr_value(stat, key) {
            section.entries.push(ReportEntry::info(label, value));
            has_entries = true;
        }
    }
    has_entries
}

fn extract_odf_content(
    kind: OdfKind,
    root: &Element,
    section: &mut ReportSection,
) -> bool {
    match kind {
        OdfKind::Text => extract_odt_content(root, section),
        OdfKind::Spreadsheet => extract_ods_content(root, section),
        OdfKind::Presentation => extract_odp_content(root, section),
        OdfKind::Unknown => false,
    }
}

fn extract_odt_content(root: &Element, section: &mut ReportSection) -> bool {
    let mut tables = 0usize;
    let mut images = 0usize;
    let mut links = 0usize;
    let mut tracked = false;
    walk_elements(root, &mut |element| {
        if element.name == "table" && namespace_matches(element, Some(TABLE_NS)) {
            tables += 1;
        }
        if element.name == "image" && namespace_matches(element, Some(DRAW_NS)) {
            images += 1;
        }
        if element.name == "a" && namespace_matches(element, Some(TEXT_NS)) {
            links += 1;
        }
        if element.name == "tracked-changes" && namespace_matches(element, Some(TEXT_NS)) {
            tracked = true;
        }
        if element.name == "changed-region" && namespace_matches(element, Some(TEXT_NS)) {
            tracked = true;
        }
    });

    let mut has_entries = false;
    if tables > 0 {
        section
            .entries
            .push(ReportEntry::info("Tablas", tables.to_string()));
        has_entries = true;
    }
    if images > 0 {
        section
            .entries
            .push(ReportEntry::info("Imagenes", images.to_string()));
        has_entries = true;
    }
    if links > 0 {
        section
            .entries
            .push(ReportEntry::info("Hipervinculos", links.to_string()));
        has_entries = true;
    }
    if tracked {
        section
            .entries
            .push(ReportEntry::warning("Control de cambios", "Si"));
        has_entries = true;
    }
    has_entries
}

fn extract_ods_content(root: &Element, section: &mut ReportSection) -> bool {
    let mut sheet_names = Vec::new();
    let mut hidden_sheets = Vec::new();
    let mut max_rows = 0u32;
    let mut max_cols = 0u32;
    let mut formulas = 0usize;

    walk_elements(root, &mut |element| {
        for (key, _) in &element.attributes {
            if key.ends_with(":formula") || key == "table:formula" {
                formulas += 1;
                break;
            }
        }
        if element.name == "table" && namespace_matches(element, Some(TABLE_NS)) {
            if let Some(name) = get_attr_value(element, "name") {
                sheet_names.push(name);
            }
            if let Some(visibility) = get_attr_value(element, "visibility") {
                if visibility == "collapse" || visibility == "hidden" {
                    if let Some(name) = get_attr_value(element, "name") {
                        hidden_sheets.push(name);
                    }
                }
            }
            let (rows, cols) = count_table_dimensions(element);
            if rows > max_rows {
                max_rows = rows;
            }
            if cols > max_cols {
                max_cols = cols;
            }
        }
    });

    let mut has_entries = false;
    if !sheet_names.is_empty() {
        section.entries.push(ReportEntry::info(
            "Hojas",
            sheet_names.len().to_string(),
        ));
        section.entries.push(ReportEntry::info(
            "Nombres de hojas",
            format_list_with_limit(&sheet_names, 10),
        ));
        has_entries = true;
    }
    if !hidden_sheets.is_empty() {
        section.entries.push(ReportEntry::warning(
            "Hojas ocultas",
            format_list_with_limit(&hidden_sheets, 10),
        ));
        has_entries = true;
    }
    if max_rows > 0 || max_cols > 0 {
        section.entries.push(ReportEntry::info(
            "Rango usado (aprox.)",
            format!("{max_rows} filas x {max_cols} columnas"),
        ));
        has_entries = true;
    }
    if formulas > 0 {
        section.entries.push(ReportEntry::info(
            "Formulas",
            formulas.to_string(),
        ));
        has_entries = true;
    }
    has_entries
}

fn extract_odp_content(root: &Element, section: &mut ReportSection) -> bool {
    let mut slides = 0usize;
    let mut notes = 0usize;
    let mut media = 0usize;

    walk_elements(root, &mut |element| {
        if element.name == "page" && namespace_matches(element, Some(DRAW_NS)) {
            if let Some(class_value) = get_attr_value(element, "class") {
                if class_value == "notes" {
                    notes += 1;
                } else {
                    slides += 1;
                }
            } else {
                slides += 1;
            }
        }
        if element.name == "notes" && namespace_matches(element, Some(PRESENTATION_NS)) {
            notes += 1;
        }
        if has_media_href(element) {
            media += 1;
        }
    });

    let mut has_entries = false;
    if slides > 0 {
        section
            .entries
            .push(ReportEntry::info("Diapositivas", slides.to_string()));
        has_entries = true;
    }
    if notes > 0 {
        section
            .entries
            .push(ReportEntry::info("Notas", notes.to_string()));
        has_entries = true;
    }
    if media > 0 {
        section
            .entries
            .push(ReportEntry::info("Medios", media.to_string()));
        has_entries = true;
    }
    has_entries
}

fn count_table_dimensions(table: &Element) -> (u32, u32) {
    let mut rows = 0u32;
    let mut max_cols = 0u32;
    for node in &table.children {
        let XMLNode::Element(child) = node else { continue };
        if child.name == "table-row" && namespace_matches(child, Some(TABLE_NS)) {
            let row_repeat = parse_repeated(child, "number-rows-repeated");
            let cols = count_row_cells(child);
            if cols > max_cols {
                max_cols = cols;
            }
            rows = rows.saturating_add(row_repeat);
        }
    }
    (rows, max_cols)
}

fn count_row_cells(row: &Element) -> u32 {
    let mut cols = 0u32;
    for node in &row.children {
        let XMLNode::Element(child) = node else { continue };
        if (child.name == "table-cell" || child.name == "covered-table-cell")
            && namespace_matches(child, Some(TABLE_NS))
        {
            cols = cols.saturating_add(parse_repeated(child, "number-columns-repeated"));
        }
    }
    cols
}

fn parse_repeated(element: &Element, key: &str) -> u32 {
    get_attr_value(element, key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
}

fn has_media_href(element: &Element) -> bool {
    for (key, value) in &element.attributes {
        if key == "href" || key.ends_with(":href") {
            let lowered = value.to_lowercase();
            if lowered.ends_with(".mp3")
                || lowered.ends_with(".wav")
                || lowered.ends_with(".m4a")
                || lowered.ends_with(".mp4")
                || lowered.ends_with(".mov")
                || lowered.ends_with(".ogg")
                || lowered.ends_with(".opus")
                || lowered.ends_with(".flac")
            {
                return true;
            }
        }
    }
    false
}

fn first_text_value(root: &Element, local: &str, namespace: Option<&str>) -> Option<String> {
    collect_text_values(root, local, namespace)
        .into_iter()
        .find(|value| !value.trim().is_empty())
}

fn collect_text_values(root: &Element, local: &str, namespace: Option<&str>) -> Vec<String> {
    let mut values = Vec::new();
    collect_text_values_inner(root, local, namespace, &mut values);
    values
}

fn collect_text_values_inner(
    element: &Element,
    local: &str,
    namespace: Option<&str>,
    values: &mut Vec<String>,
) {
    if element.name == local && namespace_matches(element, namespace) {
        let text = element_text_content(element);
        if !text.is_empty() {
            values.push(text);
        }
    }
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            collect_text_values_inner(child, local, namespace, values);
        }
    }
}

fn find_element<'a>(
    element: &'a Element,
    local: &str,
    namespace: Option<&str>,
) -> Option<&'a Element> {
    if element.name == local && namespace_matches(element, namespace) {
        return Some(element);
    }
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            if let Some(found) = find_element(child, local, namespace) {
                return Some(found);
            }
        }
    }
    None
}

fn get_attr_value<'a>(element: &'a Element, key: &str) -> Option<String> {
    for (attr_key, value) in &element.attributes {
        if attr_key == key || attr_key.ends_with(&format!(":{key}")) {
            return Some(value.to_string());
        }
    }
    None
}

fn namespace_matches(element: &Element, namespace: Option<&str>) -> bool {
    match (namespace, element.namespace.as_deref()) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn element_text_content(element: &Element) -> String {
    let mut content = String::new();
    for node in &element.children {
        if let XMLNode::Text(text) = node {
            content.push_str(text);
        }
    }
    content.trim().to_string()
}

fn walk_elements<F: FnMut(&Element)>(element: &Element, visitor: &mut F) {
    visitor(element);
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            walk_elements(child, visitor);
        }
    }
}

fn format_list_with_limit(values: &[String], limit: usize) -> String {
    if values.len() <= limit {
        return values.join(", ");
    }
    let prefix = values[..limit].join(", ");
    format!("{prefix} (+{} mas)", values.len() - limit)
}
