//! Valores compartidos para normalizar propiedades de metadata.

pub const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
pub const CP_NS: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
pub const DCTERMS_NS: &str = "http://purl.org/dc/terms/";
pub const APP_NS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

pub const CORE_SANITIZE_FIELDS: [(&str, &str); 11] = [
    ("dc:creator", ""),
    ("cp:lastModifiedBy", ""),
    ("dcterms:created", ""),
    ("dcterms:modified", ""),
    ("dc:title", ""),
    ("dc:subject", ""),
    ("dc:description", ""),
    ("cp:keywords", ""),
    ("cp:category", ""),
    ("cp:contentStatus", ""),
    ("cp:revision", "1"),
];

pub const APP_SANITIZE_FIELDS: [(&str, &str); 6] = [
    ("Application", ""),
    ("Company", ""),
    ("Manager", ""),
    ("Pages", "0"),
    ("Words", "0"),
    ("Lines", "0"),
];

pub const CUSTOM_PROPERTIES_EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"/>\n";
