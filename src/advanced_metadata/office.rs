//! Lectura de metadata en documentos Office empaquetados en ZIP.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use crate::metadata_editor::constants::{APP_NS, CP_NS, DC_NS, DCTERMS_NS};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use xmltree::{Element, XMLNode};

pub fn extract_office_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata Office");
    let mut risks = Vec::new();

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer metadata del documento Office",
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
                if is_zip { "Sí" } else { "No" },
            ));
            section.notice = Some(SectionNotice::new(
                "No se pudo leer el contenido del documento Office",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut has_entries = true;
    section.entries.push(ReportEntry::info(
        "Es ZIP",
        if is_zip { "Sí" } else { "No" },
    ));
    section
        .entries
        .push(ReportEntry::info("Entradas totales", archive.len().to_string()));
    if let Ok(comment) = std::str::from_utf8(archive.comment()) {
        if !comment.trim().is_empty() {
            section
                .entries
                .push(ReportEntry::info("Comentario ZIP", comment.trim()));
        }
    }

    let mut encrypted = archive.index_for_name("EncryptionInfo").is_some()
        || archive.index_for_name("EncryptedPackage").is_some();
    for index in 0..archive.len() {
        if let Ok(file) = archive.by_index(index) {
            if file.encrypted() {
                encrypted = true;
                break;
            }
        }
    }
    section.entries.push(ReportEntry::info(
        "Cifrado OOXML",
        if encrypted { "Sí" } else { "No" },
    ));

    let relevant_parts = collect_relevant_parts(&mut archive);
    if !relevant_parts.is_empty() {
        section.entries.push(ReportEntry::info(
            "Partes relevantes",
            relevant_parts.join(", "),
        ));
    }

    if let Ok(mut core_file) = archive.by_name("docProps/core.xml") {
        let mut contents = String::new();
        if core_file.read_to_string(&mut contents).is_ok()
            && let Some(root) = parse_xml(&contents)
        {
            has_entries |= extract_core_properties(&root, &mut section, &mut risks);
        }
    }

    if let Ok(mut app_file) = archive.by_name("docProps/app.xml") {
        let mut contents = String::new();
        if app_file.read_to_string(&mut contents).is_ok()
            && let Some(root) = parse_xml(&contents)
        {
            has_entries |= extract_app_properties(&root, &mut section, &mut risks);
        }
    }

    if let Ok(mut custom_file) = archive.by_name("docProps/custom.xml") {
        let mut contents = String::new();
        if custom_file.read_to_string(&mut contents).is_ok()
            && let Some(root) = parse_xml(&contents)
        {
            let custom_props = extract_custom_properties(&root);
            if !custom_props.is_empty() {
                for (name, value) in custom_props {
                    let label = format!("Propiedad personalizada · {}", name);
                    section.entries.push(ReportEntry::warning(&label, &value));
                    risks.push(ReportEntry::warning(label, value));
                }
            }
        }
    }

    has_entries |= extract_office_structure(&mut archive, &mut section);

    if !has_entries {
        section.notice = Some(SectionNotice::new(
            "No se encontró metadata en este documento Office",
            EntryLevel::Muted,
        ));
    } else if !risks.is_empty() {
        section.notice = Some(SectionNotice::new(
            "⚠  Este documento contiene metadata que puede revelar información personal y organizacional",
            EntryLevel::Warning,
        ));
    }

    AdvancedMetadataResult { section, risks }
}

fn parse_xml(contents: &str) -> Option<Element> {
    Element::parse(contents.as_bytes()).ok()
}

struct FieldSpec {
    label: &'static str,
    local_name: &'static str,
    namespace: Option<&'static str>,
    sensitive: bool,
}

fn extract_core_properties(
    root: &Element,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let fields = [
        FieldSpec {
            label: "Creador",
            local_name: "creator",
            namespace: Some(DC_NS),
            sensitive: true,
        },
        FieldSpec {
            label: "Última modificación por",
            local_name: "lastModifiedBy",
            namespace: Some(CP_NS),
            sensitive: true,
        },
        FieldSpec {
            label: "Fecha de creación",
            local_name: "created",
            namespace: Some(DCTERMS_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Fecha de modificación",
            local_name: "modified",
            namespace: Some(DCTERMS_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Título",
            local_name: "title",
            namespace: Some(DC_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Asunto",
            local_name: "subject",
            namespace: Some(DC_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Descripción",
            local_name: "description",
            namespace: Some(DC_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Palabras clave",
            local_name: "keywords",
            namespace: Some(CP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Categoría",
            local_name: "category",
            namespace: Some(CP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Estado de contenido",
            local_name: "contentStatus",
            namespace: Some(CP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Revisión",
            local_name: "revision",
            namespace: Some(CP_NS),
            sensitive: false,
        },
    ];

    extract_fields(root, &fields, section, risks)
}

fn extract_app_properties(
    root: &Element,
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let fields = [
        FieldSpec {
            label: "Aplicación",
            local_name: "Application",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Versión de aplicación",
            local_name: "AppVersion",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Plantilla",
            local_name: "Template",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Empresa",
            local_name: "Company",
            namespace: Some(APP_NS),
            sensitive: true,
        },
        FieldSpec {
            label: "Administrador",
            local_name: "Manager",
            namespace: Some(APP_NS),
            sensitive: true,
        },
        FieldSpec {
            label: "Páginas",
            local_name: "Pages",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Párrafos",
            local_name: "Paragraphs",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Palabras",
            local_name: "Words",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Líneas",
            local_name: "Lines",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Caracteres",
            local_name: "Characters",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Caracteres (con espacios)",
            local_name: "CharactersWithSpaces",
            namespace: Some(APP_NS),
            sensitive: false,
        },
        FieldSpec {
            label: "Tiempo total",
            local_name: "TotalTime",
            namespace: Some(APP_NS),
            sensitive: false,
        },
    ];

    extract_fields(root, &fields, section, risks)
}

fn extract_fields(
    root: &Element,
    fields: &[FieldSpec],
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
) -> bool {
    let mut found = false;
    for field in fields {
        if let Some(value) = find_child_text(root, field.local_name, field.namespace) {
            let level = if field.sensitive {
                EntryLevel::Warning
            } else {
                EntryLevel::Info
            };
            section
                .entries
                .push(ReportEntry::new(field.label, &value, level));
            if field.sensitive {
                risks.push(ReportEntry::warning(field.label, value));
            }
            found = true;
        }
    }
    found
}

fn find_child_text(root: &Element, local_name: &str, namespace: Option<&str>) -> Option<String> {
    for node in &root.children {
        if let XMLNode::Element(child) = node
            && child.name == local_name
            && namespace_matches(child, namespace)
        {
            return Some(element_text_content(child));
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

fn extract_custom_properties(root: &Element) -> Vec<(String, String)> {
    let mut props = Vec::new();
    for node in &root.children {
        if let XMLNode::Element(child) = node {
            if child.name != "property" {
                continue;
            }
            let name = match child.attributes.get("name") {
                Some(name) if !name.trim().is_empty() => name.to_string(),
                _ => continue,
            };
            let value = child
                .children
                .iter()
                .find_map(|node| match node {
                    XMLNode::Element(value_node) => Some(element_text_content(value_node)),
                    _ => None,
                })
                .unwrap_or_default();
            props.push((name, value));
        }
    }
    props
}

fn collect_relevant_parts(archive: &mut zip::ZipArchive<File>) -> Vec<String> {
    let mut parts = Vec::new();
    let candidates = [
        "docProps/core.xml",
        "docProps/app.xml",
        "docProps/custom.xml",
        "[Content_Types].xml",
    ];
    for name in candidates {
        if archive.index_for_name(name).is_some() {
            parts.push(name.to_string());
        }
    }

    let mut has_word = false;
    let mut has_xl = false;
    let mut has_ppt = false;
    for name in archive.file_names() {
        if name.starts_with("word/") {
            has_word = true;
        } else if name.starts_with("xl/") {
            has_xl = true;
        } else if name.starts_with("ppt/") {
            has_ppt = true;
        }
    }
    if has_word {
        parts.push("word/*".to_string());
    }
    if has_xl {
        parts.push("xl/*".to_string());
    }
    if has_ppt {
        parts.push("ppt/*".to_string());
    }
    parts
}

fn extract_office_structure(
    archive: &mut zip::ZipArchive<File>,
    section: &mut ReportSection,
) -> bool {
    let mut has_entries = false;
    if archive.index_for_name("word/document.xml").is_some() {
        has_entries |= extract_docx_structure(archive, section);
    }
    if archive.index_for_name("xl/workbook.xml").is_some() {
        has_entries |= extract_xlsx_structure(archive, section);
    }
    if archive.index_for_name("ppt/presentation.xml").is_some() {
        has_entries |= extract_pptx_structure(archive, section);
    }
    has_entries
}

fn extract_docx_structure(
    archive: &mut zip::ZipArchive<File>,
    section: &mut ReportSection,
) -> bool {
    let Some(contents) = read_zip_string(archive, "word/document.xml") else {
        return false;
    };
    let Some(root) = parse_xml(&contents) else {
        return false;
    };
    let sections = count_elements(&root, "sectPr");
    let tables = count_elements(&root, "tbl");
    let hyperlinks = count_elements(&root, "hyperlink");
    let drawings = count_elements(&root, "blip");
    let fields = count_elements(&root, "fldSimple") + count_elements(&root, "instrText");
    let tracked = count_elements(&root, "ins") + count_elements(&root, "del");

    section
        .entries
        .push(ReportEntry::info("Secciones", sections.to_string()));
    section
        .entries
        .push(ReportEntry::info("Tablas", tables.to_string()));
    section
        .entries
        .push(ReportEntry::info("Imágenes embebidas", drawings.to_string()));
    section.entries.push(ReportEntry::info(
        "Hipervínculos",
        hyperlinks.to_string(),
    ));
    section
        .entries
        .push(ReportEntry::info("Campos", fields.to_string()));
    section.entries.push(ReportEntry::info(
        "Control de cambios",
        if tracked > 0 { "Sí" } else { "No" },
    ));

    if let Some(comments) = read_zip_string(archive, "word/comments.xml") {
        if let Some(root) = parse_xml(&comments) {
            let count = count_elements(&root, "comment");
            section
                .entries
                .push(ReportEntry::info("Comentarios", count.to_string()));
        }
    }

    let has_macros = archive.index_for_name("word/vbaProject.bin").is_some();
    section.entries.push(ReportEntry::info(
        "Macros",
        if has_macros { "Sí" } else { "No" },
    ));
    true
}

fn extract_xlsx_structure(
    archive: &mut zip::ZipArchive<File>,
    section: &mut ReportSection,
) -> bool {
    let Some(contents) = read_zip_string(archive, "xl/workbook.xml") else {
        return false;
    };
    let Some(root) = parse_xml(&contents) else {
        return false;
    };

    let mut sheet_names = Vec::new();
    let mut hidden_sheets = Vec::new();
    for node in &root.children {
        if let XMLNode::Element(child) = node
            && child.name == "sheets"
        {
            for node in &child.children {
                if let XMLNode::Element(sheet) = node
                    && sheet.name == "sheet"
                {
                    if let Some(name) = sheet.attributes.get("name") {
                        sheet_names.push(name.to_string());
                        if let Some(state) = sheet.attributes.get("state") {
                            if state == "hidden" || state == "veryHidden" {
                                hidden_sheets.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    section.entries.push(ReportEntry::info(
        "Hojas",
        sheet_names.len().to_string(),
    ));
    if !sheet_names.is_empty() {
        section.entries.push(ReportEntry::info(
            "Nombres de hojas",
            sheet_names.join(", "),
        ));
    }
    if !hidden_sheets.is_empty() {
        section.entries.push(ReportEntry::info(
            "Hojas ocultas",
            hidden_sheets.join(", "),
        ));
    } else {
        section
            .entries
            .push(ReportEntry::info("Hojas ocultas", "No"));
    }

    let mut formula_count = 0;
    let mut used_ranges = Vec::new();
    let mut protected_sheets = 0;
    let sheet_files = archive
        .file_names()
        .filter(|name| name.starts_with("xl/worksheets/"))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    for name in sheet_files {
        if let Some(sheet_xml) = read_zip_string(archive, &name) {
            if let Some(sheet_root) = parse_xml(&sheet_xml) {
                if let Some(dimension) = find_child_attribute(&sheet_root, "dimension", "ref") {
                    used_ranges.push(dimension);
                }
                formula_count += count_elements(&sheet_root, "f");
                if count_elements(&sheet_root, "sheetProtection") > 0 {
                    protected_sheets += 1;
                }
            }
        }
    }
    if !used_ranges.is_empty() {
        section.entries.push(ReportEntry::info(
            "Rangos usados",
            used_ranges.join(", "),
        ));
    }
    section.entries.push(ReportEntry::info(
        "Fórmulas",
        formula_count.to_string(),
    ));
    if protected_sheets > 0 {
        section.entries.push(ReportEntry::info(
            "Hojas protegidas",
            protected_sheets.to_string(),
        ));
    } else {
        section
            .entries
            .push(ReportEntry::info("Hojas protegidas", "No"));
    }

    let tables = archive
        .file_names()
        .filter(|name| name.starts_with("xl/tables/"))
        .count();
    section
        .entries
        .push(ReportEntry::info("Tablas", tables.to_string()));
    let charts = archive
        .file_names()
        .filter(|name| name.starts_with("xl/charts/"))
        .count();
    section
        .entries
        .push(ReportEntry::info("Gráficos", charts.to_string()));
    let pivots = archive
        .file_names()
        .filter(|name| name.starts_with("xl/pivotTables/"))
        .count();
    section
        .entries
        .push(ReportEntry::info("Pivots", pivots.to_string()));
    let external_links = archive
        .file_names()
        .filter(|name| name.starts_with("xl/externalLinks/"))
        .count();
    section.entries.push(ReportEntry::info(
        "Vínculos externos",
        external_links.to_string(),
    ));
    let connections = archive.index_for_name("xl/connections.xml").is_some();
    section.entries.push(ReportEntry::info(
        "Conexiones externas",
        if connections { "Sí" } else { "No" },
    ));

    let workbook_protected = count_elements(&root, "workbookProtection") > 0;
    section.entries.push(ReportEntry::info(
        "Workbook protegido",
        if workbook_protected { "Sí" } else { "No" },
    ));

    let has_macros = archive.index_for_name("xl/vbaProject.bin").is_some();
    section.entries.push(ReportEntry::info(
        "Macros",
        if has_macros { "Sí" } else { "No" },
    ));

    true
}

fn extract_pptx_structure(
    archive: &mut zip::ZipArchive<File>,
    section: &mut ReportSection,
) -> bool {
    let Some(contents) = read_zip_string(archive, "ppt/presentation.xml") else {
        return false;
    };
    let Some(root) = parse_xml(&contents) else {
        return false;
    };

    let slides = count_elements(&root, "sldId");
    section
        .entries
        .push(ReportEntry::info("Diapositivas", slides.to_string()));

    let notes = archive
        .file_names()
        .filter(|name| name.starts_with("ppt/notesSlides/"))
        .count();
    section
        .entries
        .push(ReportEntry::info("Notas", notes.to_string()));

    let mut images = 0;
    let mut media = 0;
    for name in archive.file_names().filter(|name| name.starts_with("ppt/media/")) {
        if name.ends_with(".png")
            || name.ends_with(".jpg")
            || name.ends_with(".jpeg")
            || name.ends_with(".gif")
        {
            images += 1;
        } else if name.ends_with(".mp3")
            || name.ends_with(".wav")
            || name.ends_with(".mp4")
            || name.ends_with(".m4a")
        {
            media += 1;
        }
    }
    section
        .entries
        .push(ReportEntry::info("Imágenes", images.to_string()));
    section
        .entries
        .push(ReportEntry::info("Audio/Video", media.to_string()));

    let mut transitions = 0;
    let mut hyperlinks = 0;
    let slide_files = archive
        .file_names()
        .filter(|name| name.starts_with("ppt/slides/"))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    for name in slide_files {
        if let Some(slide_xml) = read_zip_string(archive, &name) {
            if let Some(slide_root) = parse_xml(&slide_xml) {
                transitions += count_elements(&slide_root, "transition");
                hyperlinks += count_elements(&slide_root, "hlinkClick");
            }
        }
    }
    section.entries.push(ReportEntry::info(
        "Transiciones/animaciones",
        transitions.to_string(),
    ));
    section.entries.push(ReportEntry::info(
        "Hipervínculos",
        hyperlinks.to_string(),
    ));

    let has_macros = archive.index_for_name("ppt/vbaProject.bin").is_some();
    section.entries.push(ReportEntry::info(
        "Macros",
        if has_macros { "Sí" } else { "No" },
    ));

    true
}

fn read_zip_string(archive: &mut zip::ZipArchive<File>, name: &str) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    Some(contents)
}

fn count_elements(root: &Element, name: &str) -> usize {
    let mut count = 0;
    for node in &root.children {
        if let XMLNode::Element(child) = node {
            if child.name == name {
                count += 1;
            }
            count += count_elements(child, name);
        }
    }
    count
}

fn find_child_attribute(root: &Element, name: &str, attr: &str) -> Option<String> {
    for node in &root.children {
        if let XMLNode::Element(child) = node {
            if child.name == name {
                if let Some(value) = child.attributes.get(attr) {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}
