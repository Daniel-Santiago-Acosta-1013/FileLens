use crate::metadata::report::{EntryLevel, ReportEntry};
use std::collections::HashSet;
use xmltree::{Element, XMLNode};

const MAX_XMP_VALUE_LEN: usize = 2048;

pub struct XmpMetadata {
    pub entries: Vec<ReportEntry>,
    pub risks: Vec<ReportEntry>,
    pub gps_position: Option<String>,
}

pub fn parse_xmp_metadata(packet: &str) -> Option<XmpMetadata> {
    let xml = extract_xmp_xml(packet)?;
    let root = Element::parse(xml.as_bytes()).ok()?;

    let mut metadata = XmpMetadata {
        entries: Vec::new(),
        risks: Vec::new(),
        gps_position: None,
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
        XmpFieldSpec {
            label: "GPS Latitud",
            keys: &["exif:GPSLatitude", "GPSLatitude"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Longitud",
            keys: &["exif:GPSLongitude", "GPSLongitude"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Altitud",
            keys: &["exif:GPSAltitude", "GPSAltitude"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Velocidad",
            keys: &["exif:GPSSpeed", "GPSSpeed"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Rumbo",
            keys: &["exif:GPSTrack", "GPSTrack"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Dirección",
            keys: &["exif:GPSImgDirection", "GPSImgDirection"],
            sensitive: true,
        },
        XmpFieldSpec {
            label: "GPS Datum",
            keys: &["exif:GPSMapDatum", "GPSMapDatum"],
            sensitive: true,
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

    metadata.gps_position = build_gps_position(&root);

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

fn build_gps_position(root: &Element) -> Option<String> {
    let lat = first_value(root, &["exif:GPSLatitude", "GPSLatitude"])?;
    let lon = first_value(root, &["exif:GPSLongitude", "GPSLongitude"])?;
    let lat_ref = first_value(root, &["exif:GPSLatitudeRef", "GPSLatitudeRef"]);
    let lon_ref = first_value(root, &["exif:GPSLongitudeRef", "GPSLongitudeRef"]);

    let lat_formatted = format_gps_value(&lat, lat_ref.as_deref(), true)?;
    let lon_formatted = format_gps_value(&lon, lon_ref.as_deref(), false)?;

    Some(format!("{lat_formatted}, {lon_formatted}"))
}

fn first_value(root: &Element, keys: &[&str]) -> Option<String> {
    let mut values = Vec::new();
    for key in keys {
        collect_values_for_key(root, key, &mut values);
    }
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
}

fn format_gps_value(value: &str, ref_override: Option<&str>, is_lat: bool) -> Option<String> {
    let mut reference = ref_override
        .and_then(find_ref_char)
        .or_else(|| find_ref_char(value));

    let (deg, min, sec) = match extract_numbers(value).as_slice() {
        [deg, min, sec, ..] => (*deg, *min, *sec),
        [decimal] => {
            if *decimal < 0.0 && reference.is_none() {
                reference = Some(if is_lat { 'S' } else { 'W' });
            }
            decimal_to_dms(*decimal)
        }
        _ => return None,
    };

    let (deg, min, sec) = normalize_dms(deg.abs(), min.abs(), sec.abs());
    let deg_label = format_decimal(deg, 0);
    let min_label = format_decimal(min, 0);
    let sec_label = format_decimal(sec, 2);
    let suffix = reference.map(|c| format!(" {c}")).unwrap_or_default();
    Some(format!("{deg_label} grados {min_label}' {sec_label}\"{suffix}"))
}

fn decimal_to_dms(value: f64) -> (f64, f64, f64) {
    let abs = value.abs();
    let deg = abs.floor();
    let minutes_total = (abs - deg) * 60.0;
    let min = minutes_total.floor();
    let sec = (minutes_total - min) * 60.0;
    (deg, min, sec)
}

fn normalize_dms(degrees: f64, minutes: f64, seconds: f64) -> (f64, f64, f64) {
    let mut deg = degrees;
    let mut min = minutes;
    let mut sec = seconds;
    if sec >= 60.0 {
        min += (sec / 60.0).floor();
        sec = sec % 60.0;
    }
    if min >= 60.0 {
        deg += (min / 60.0).floor();
        min = min % 60.0;
    }
    (deg, min, sec)
}

fn format_decimal(value: f64, decimals: usize) -> String {
    let mut out = format!("{:.*}", decimals, value);
    if out.contains('.') {
        out = out.replace('.', ",");
    }
    out
}

fn extract_numbers(value: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut buffer = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == '-' {
            if ch == ',' {
                buffer.push('.');
            } else {
                buffer.push(ch);
            }
        } else if !buffer.is_empty() {
            if let Ok(parsed) = buffer.parse::<f64>() {
                numbers.push(parsed);
            }
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        if let Ok(parsed) = buffer.parse::<f64>() {
            numbers.push(parsed);
        }
    }
    numbers
}

fn find_ref_char(value: &str) -> Option<char> {
    value.chars().find_map(|ch| match ch.to_ascii_uppercase() {
        'N' | 'S' | 'E' | 'W' => Some(ch.to_ascii_uppercase()),
        _ => None,
    })
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
