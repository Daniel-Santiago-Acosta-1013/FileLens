use crate::metadata::report::{EntryLevel, ReportEntry};
use std::collections::HashSet;
use xmltree::{Element, XMLNode};

const MAX_XMP_VALUE_LEN: usize = 2048;

pub struct XmpMetadata {
    pub entries: Vec<ReportEntry>,
    pub risks: Vec<ReportEntry>,
}

pub fn parse_xmp_metadata(packet: &str) -> Option<XmpMetadata> {
    let xml = extract_xmp_xml(packet)?;
    let root = Element::parse(xml.as_bytes()).ok()?;

    let mut metadata = XmpMetadata {
        entries: Vec::new(),
        risks: Vec::new(),
    };
    let mut seen = HashSet::new();

    if let Some(toolkit) = find_attribute_value(&root, "xmptk") {
        push_entry(
            &mut metadata.entries,
            &mut seen,
            "XMP Toolkit",
            toolkit,
            EntryLevel::Info,
        );
    }

    let specs = [
        XmpFieldSpec {
            label: "XMP Creador",
            keys: &["dc:creator", "creator"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "XMP Título",
            keys: &["dc:title", "title"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Descripción",
            keys: &["dc:description", "description"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Palabras clave",
            keys: &["dc:subject", "subject"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Derechos",
            keys: &["dc:rights", "rights"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "XMP Licencia",
            keys: &["xmpRights:UsageTerms", "cc:license", "license"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "XMP Herramienta",
            keys: &["xmp:CreatorTool", "CreatorTool"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Fecha de creación",
            keys: &["xmp:CreateDate", "CreateDate"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Fecha de modificación",
            keys: &["xmp:ModifyDate", "ModifyDate"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Fecha de metadata",
            keys: &["xmp:MetadataDate", "MetadataDate"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Rating",
            keys: &["xmp:Rating", "Rating"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Label",
            keys: &["xmp:Label", "Label"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Productor PDF",
            keys: &["pdf:Producer", "Producer"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Palabras clave PDF",
            keys: &["pdf:Keywords", "Keywords"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Identificador",
            keys: &["xmpMM:DocumentID", "DocumentID"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Instancia",
            keys: &["xmpMM:InstanceID", "InstanceID"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Historial",
            keys: &["xmpMM:History", "photoshop:History", "History"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Ancestros",
            keys: &["photoshop:DocumentAncestors", "DocumentAncestors"],
            sensitive: false,
        },
        XmpFieldSpec {
            label: "XMP Información de edición",
            keys: &["photoshop:Credit", "photoshop:Source", "xmpMM:DerivedFrom"],
            sensitive: false,
        },
    ];

    for spec in specs {
        let value = collect_values(&root, spec.keys);
        if value.is_empty() {
            continue;
        }
        let level = if spec.sensitive {
            EntryLevel::Warning
        } else {
            EntryLevel::Info
        };
        if push_entry(&mut metadata.entries, &mut seen, spec.label, value.clone(), level)
            && spec.sensitive
        {
            metadata.risks.push(ReportEntry::warning(spec.label, value));
        }
    }

    Some(metadata)
}

struct XmpFieldSpec {
    label: &'static str,
    keys: &'static [&'static str],
    sensitive: bool,
}

fn extract_xmp_xml(packet: &str) -> Option<String> {
    if let Some(xml) = slice_between(packet, "<x:xmpmeta", "</x:xmpmeta>") {
        return Some(xml.to_string());
    }
    if let Some(xml) = slice_between(packet, "<rdf:RDF", "</rdf:RDF>") {
        return Some(xml.to_string());
    }
    if packet.contains("<xmpmeta") || packet.contains("<rdf:RDF") {
        return Some(packet.to_string());
    }
    None
}

fn slice_between<'a>(value: &'a str, start_tag: &str, end_tag: &str) -> Option<&'a str> {
    let start = value.find(start_tag)?;
    let end = value[start..].find(end_tag)?;
    let end_index = start + end + end_tag.len();
    Some(&value[start..end_index])
}

fn find_attribute_value(root: &Element, key: &str) -> Option<String> {
    let mut values = Vec::new();
    collect_attribute_values(root, key, &mut values);
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
}

fn collect_values(root: &Element, keys: &[&str]) -> String {
    let mut collected = Vec::new();
    for key in keys {
        collect_values_for_key(root, key, &mut collected);
    }

    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for value in collected {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() || trimmed.len() > MAX_XMP_VALUE_LEN {
            continue;
        }
        if seen.insert(trimmed.clone()) {
            values.push(trimmed);
        }
    }
    values.join(", ")
}

fn collect_values_for_key(root: &Element, key: &str, values: &mut Vec<String>) {
    for (attr_key, attr_value) in &root.attributes {
        if key_matches(attr_key, key) {
            values.push(attr_value.to_string());
        }
    }

    let name = qualified_name(root);
    if key_matches(&name, key) {
        let text = element_text(root);
        if !text.is_empty() {
            values.push(text);
        }
    }

    for node in &root.children {
        if let XMLNode::Element(child) = node {
            collect_values_for_key(child, key, values);
        }
    }
}

fn collect_attribute_values(root: &Element, key: &str, values: &mut Vec<String>) {
    for (attr_key, attr_value) in &root.attributes {
        if key_matches(attr_key, key) {
            values.push(attr_value.to_string());
        }
    }
    for node in &root.children {
        if let XMLNode::Element(child) = node {
            collect_attribute_values(child, key, values);
        }
    }
}

fn qualified_name(element: &Element) -> String {
    if let Some(prefix) = &element.prefix {
        format!("{prefix}:{}", element.name)
    } else {
        element.name.clone()
    }
}

fn element_text(element: &Element) -> String {
    let mut parts = Vec::new();
    collect_text_nodes(element, &mut parts);
    parts
        .into_iter()
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn collect_text_nodes(element: &Element, values: &mut Vec<String>) {
    for node in &element.children {
        match node {
            XMLNode::Text(text) => values.push(text.trim().to_string()),
            XMLNode::Element(child) => collect_text_nodes(child, values),
            _ => {}
        }
    }
}

fn key_matches(found: &str, wanted: &str) -> bool {
    if found.eq_ignore_ascii_case(wanted) {
        return true;
    }
    if !wanted.contains(':') && let Some(local) = found.rsplit(':').next() {
        return local.eq_ignore_ascii_case(wanted);
    }
    false
}

fn push_entry(
    entries: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    label: &str,
    value: String,
    level: EntryLevel,
) -> bool {
    if !seen.insert(label.to_string()) {
        return false;
    }
    entries.push(ReportEntry::new(label, value, level));
    true
}
