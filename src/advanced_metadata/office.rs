//! Lectura de metadata en documentos Office empaquetados en ZIP.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use crate::metadata_editor::constants::{APP_NS, CP_NS, DC_NS, DCTERMS_NS};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use xmltree::{Element, XMLNode};

pub fn extract_office_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata Office");
    let mut risks = Vec::new();

    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer metadata del documento Office",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer el contenido del documento Office",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut has_entries = false;

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
                has_entries = true;
            }
        }
    }

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
