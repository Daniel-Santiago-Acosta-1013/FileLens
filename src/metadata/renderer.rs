//! Reúne la metadata del archivo solicitado.

use crate::advanced_metadata::{
    extract_csv_metadata, extract_image_metadata, extract_media_metadata, extract_odf_metadata,
    extract_office_metadata, extract_pdf_metadata, extract_text_metadata, extract_zip_metadata,
};
use crate::directory::{count_directory_entries, EntryKind};
use crate::formatting::{format_optional_time, format_size};
use std::fs;
use std::io::Read;
use std::path::Path;

use super::hashing::file_hashes;
use super::mime::{detect_file_type, DetectedFileType};
use super::report::{MetadataOptions, MetadataReport, ReportEntry, ReportSection};

pub fn build_report(path: &Path, options: &MetadataOptions) -> Result<MetadataReport, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "No se pudo obtener la metadata de `{}`: {error}",
            path.display()
        )
    })?;

    let kind = EntryKind::from(&metadata);
    let detected = if matches!(kind, EntryKind::File) {
        detect_file_type(path)
    } else {
        DetectedFileType {
            mime: None,
            extension: None,
        }
    };
    let mime = detected.mime.as_deref();
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase());
    let extension_hint = extension.as_deref().or(detected.extension.as_deref());

    let mut report = MetadataReport::new();
    report.system.extend(collect_path_details(path));
    report.system.extend(collect_name_details(path));
    report.system.extend(collect_kind_details(&metadata, &kind));

    if let Some(entry) = collect_directory_summary(path, &kind) {
        report.system.push(entry);
    }

    report.system.extend(collect_permissions(&metadata));
    report.system.extend(collect_file_specifics(
        path,
        &metadata,
        &kind,
        mime,
        extension.as_deref(),
        &detected,
        options,
    ));
    report.system.extend(collect_timestamps(&metadata));

    if let Some(entry) = collect_symlink_target(path, &metadata) {
        report.system.push(entry);
    }

    let (sections, risks) = collect_advanced_metadata(path, &kind, mime, extension_hint);
    report.internal = sections;
    report.risks = risks;

    Ok(report)
}

fn collect_path_details(path: &Path) -> Vec<ReportEntry> {
    let mut entries = Vec::new();
    entries.push(ReportEntry::info(
        "Ruta ingresada",
        path.display().to_string(),
    ));

    let canonical = fs::canonicalize(path)
        .map(|real_path| real_path.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    entries.push(ReportEntry::info("Ruta resuelta", canonical));
    entries
}

fn collect_name_details(path: &Path) -> Vec<ReportEntry> {
    let mut entries = Vec::new();
    if let Some(name) = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
    {
        entries.push(ReportEntry::info("Nombre", name));
    }

    if let Some(ext) = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
    {
        entries.push(ReportEntry::info("Extensión", ext));
    }
    entries
}

fn collect_kind_details(metadata: &fs::Metadata, kind: &EntryKind) -> Vec<ReportEntry> {
    let mut entries = Vec::new();
    entries.push(ReportEntry::info("Tipo", kind_label(kind)));

    let size_str = match kind {
        EntryKind::File => format_size(metadata.len()),
        _ => format!("{} bytes", metadata.len()),
    };
    entries.push(ReportEntry::info("Tamaño", size_str));
    entries
}

fn collect_directory_summary(path: &Path, kind: &EntryKind) -> Option<ReportEntry> {
    if !matches!(kind, EntryKind::Directory) {
        return None;
    }

    if let Ok((count, truncated)) = count_directory_entries(path) {
        let label = if truncated {
            format!("{count}+ elementos directos")
        } else {
            format!("{count} elementos directos")
        };
        return Some(ReportEntry::info("Contenido", label));
    }

    None
}

fn collect_permissions(metadata: &fs::Metadata) -> Vec<ReportEntry> {
    let mut entries = Vec::new();
    let readonly = metadata.permissions().readonly();
    let readonly_value = if readonly {
        "Solo lectura"
    } else {
        "Lectura y escritura"
    };
    let entry = if readonly {
        ReportEntry::warning("Permisos", readonly_value)
    } else {
        ReportEntry::success("Permisos", readonly_value)
    };
    entries.push(entry);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        entries.push(ReportEntry::info(
            "Permisos (octal)",
            format!("{:04o}", metadata.permissions().mode() & 0o7777),
        ));
        entries.push(ReportEntry::info(
            "Permisos (rwx)",
            super::permissions::format_unix_permissions(metadata.permissions().mode()),
        ));

        let owner =
            super::permissions::owner_name(metadata).unwrap_or_else(|| "Desconocido".to_string());
        entries.push(ReportEntry::info("Propietario", owner));

        let group =
            super::permissions::group_name(metadata).unwrap_or_else(|| "Desconocido".to_string());
        entries.push(ReportEntry::info("Grupo", group));
    }

    entries
}

fn collect_file_specifics(
    path: &Path,
    metadata: &fs::Metadata,
    kind: &EntryKind,
    mime: Option<&str>,
    extension: Option<&str>,
    detected: &DetectedFileType,
    options: &MetadataOptions,
) -> Vec<ReportEntry> {
    if !matches!(kind, EntryKind::File) {
        return Vec::new();
    }

    let mut entries = Vec::new();

    if let Some(label) = file_type_label(mime, extension, detected.extension.as_deref()) {
        entries.push(ReportEntry::info("Tipo de archivo", label));
    }

    if let Some(ext) = extension.or(detected.extension.as_deref()) {
        entries.push(ReportEntry::info("Extensión del tipo de archivo", ext));
    }

    if let Some(mime) = mime {
        entries.push(ReportEntry::info("Tipo MIME", mime));
    }

    if let Some(category) = category_for(mime, extension) {
        entries.push(ReportEntry::info("Categoría", category));
    }

    if let Some(header) = read_file_header(path) {
        entries.push(ReportEntry::info("Encabezado (hex)", header));
    }

    entries.push(ReportEntry::info(
        "Tamaño (bytes)",
        metadata.len().to_string(),
    ));

    if options.include_hash {
        let hashes = file_hashes(path, metadata);
        entries.push(ReportEntry::info("Hash MD5", hashes.md5));
        entries.push(ReportEntry::info("Hash SHA-256", hashes.sha256));
    } else {
        entries.push(ReportEntry::info("Hash MD5", "Omitido (desactivado)"));
        entries.push(ReportEntry::info("Hash SHA-256", "Omitido (desactivado)"));
    }

    entries
}

fn read_file_header(path: &Path) -> Option<String> {
    const HEADER_LIMIT: usize = 64;
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = [0_u8; HEADER_LIMIT];
    let bytes_read = file.read(&mut buffer).ok()?;
    if bytes_read == 0 {
        return None;
    }
    let header = buffer[..bytes_read]
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join(" ");
    Some(header)
}

fn collect_timestamps(metadata: &fs::Metadata) -> Vec<ReportEntry> {
    vec![
        ReportEntry::info(
            "Último acceso",
            format_optional_time(metadata.accessed().ok()),
        ),
        ReportEntry::info(
            "Última modificación",
            format_optional_time(metadata.modified().ok()),
        ),
        ReportEntry::info(
            "Fecha de creación",
            format_optional_time(metadata.created().ok()),
        ),
    ]
}

fn collect_symlink_target(path: &Path, metadata: &fs::Metadata) -> Option<ReportEntry> {
    if !metadata.file_type().is_symlink() {
        return None;
    }

    let target = fs::read_link(path)
        .map(|t| t.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    Some(ReportEntry::info("Enlace simbólico a", target))
}

fn collect_advanced_metadata(
    path: &Path,
    kind: &EntryKind,
    mime: Option<&str>,
    extension: Option<&str>,
) -> (Vec<ReportSection>, Vec<ReportEntry>) {
    if !matches!(kind, EntryKind::File) {
        return (Vec::new(), Vec::new());
    }

    let mut sections = Vec::new();
    let mut risks = Vec::new();

    if is_image(mime, extension) {
        let result = extract_image_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_pdf(mime, extension) {
        let result = extract_pdf_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_office(mime, extension) {
        let result = extract_office_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_odf(mime, extension) {
        let result = extract_odf_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_csv(mime, extension) {
        let result = extract_csv_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    } else if is_text(mime, extension) {
        let result = extract_text_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_media(mime, extension) {
        let result = extract_media_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    if is_zip(mime, extension) && !is_office(mime, extension) && !is_odf(mime, extension) {
        let result = extract_zip_metadata(path);
        sections.push(result.section);
        risks.extend(result.risks);
    }

    (sections, risks)
}

fn is_image(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some(m) if m.starts_with("image/"))
        || matches!(
            extension,
            Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "tiff" | "tif" | "heic" | "heif" | "svg")
        )
}

fn is_pdf(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some("application/pdf")) || matches!(extension, Some("pdf"))
}

fn is_office(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some(m) if m.contains("officedocument") || m.contains("msword") || m.contains("ms-excel") || m.contains("ms-powerpoint"))
        || matches!(extension, Some("docx" | "xlsx" | "pptx"))
}

fn is_odf(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some(m) if m.contains("opendocument"))
        || matches!(extension, Some("odt" | "ods" | "odp"))
}

fn is_zip(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some("application/zip")) || matches!(extension, Some("zip"))
}

fn is_text(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some("text/plain")) || matches!(extension, Some("txt"))
}

fn is_csv(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some("text/csv")) || matches!(extension, Some("csv"))
}

fn is_media(mime: Option<&str>, extension: Option<&str>) -> bool {
    matches!(mime, Some(m) if m.starts_with("audio/") || m.starts_with("video/"))
        || matches!(
            extension,
            Some(
                "mp3" | "wav" | "flac" | "ogg" | "opus" | "m4a" | "mp4" | "mov" | "mkv"
            )
        )
}

fn file_type_label(
    mime: Option<&str>,
    extension: Option<&str>,
    detected_extension: Option<&str>,
) -> Option<String> {
    if let Some(mime) = mime {
        let label = match mime {
            "image/jpeg" => "JPEG",
            "image/png" => "PNG",
            "image/gif" => "GIF",
            "image/webp" => "WebP",
            "image/tiff" => "TIFF",
            "image/heic" | "image/heif" | "image/avif" => "HEIF",
            "image/svg+xml" => "SVG",
            "application/pdf" => "PDF",
            "application/zip" => "ZIP",
            "audio/mpeg" => "MP3",
            "audio/mp4" | "audio/x-m4a" => "M4A",
            "audio/wav" | "audio/x-wav" => "WAV",
            "audio/flac" | "audio/x-flac" => "FLAC",
            "audio/ogg" | "application/ogg" => "OGG",
            "audio/opus" => "OPUS",
            "video/mp4" => "MP4",
            "video/quicktime" => "MOV",
            "video/x-matroska" => "MKV",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "DOCX",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "XLSX",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "PPTX",
            "application/vnd.oasis.opendocument.text" => "ODT",
            "application/vnd.oasis.opendocument.spreadsheet" => "ODS",
            "application/vnd.oasis.opendocument.presentation" => "ODP",
            "text/plain" => "TXT",
            "text/csv" => "CSV",
            _ => "",
        };
        if !label.is_empty() {
            return Some(label.to_string());
        }
    }

    let ext = extension.or(detected_extension)?;
    Some(ext.to_ascii_uppercase())
}

fn category_for(mime: Option<&str>, extension: Option<&str>) -> Option<&'static str> {
    if let Some(mime) = mime {
        if mime.starts_with("image/") {
            return Some("Imagen");
        }
        if mime.starts_with("audio/") {
            return Some("Audio");
        }
        if mime.starts_with("video/") {
            return Some("Video");
        }
        if mime == "application/zip" {
            return Some("Archivo comprimido");
        }
        if mime == "application/pdf"
            || mime.contains("officedocument")
            || mime.contains("msword")
            || mime.contains("ms-excel")
            || mime.contains("ms-powerpoint")
            || mime.contains("opendocument")
            || mime.starts_with("text/")
        {
            return Some("Documento");
        }
    }

    match extension {
        Some(
            "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "webp"
            | "tiff"
            | "tif"
            | "heic"
            | "heif"
            | "svg",
        ) => Some("Imagen"),
        Some("mp3" | "wav" | "flac" | "ogg" | "opus" | "m4a") => Some("Audio"),
        Some("mp4" | "mov" | "mkv") => Some("Video"),
        Some("zip") => Some("Archivo comprimido"),
        Some(
            "pdf"
            | "docx"
            | "xlsx"
            | "pptx"
            | "odt"
            | "ods"
            | "odp"
            | "txt"
            | "csv",
        ) => Some("Documento"),
        _ => None,
    }
}

fn kind_label(kind: &EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "Directorio",
        EntryKind::File => "Archivo",
        EntryKind::Symlink => "Enlace simbólico",
        EntryKind::Other => "Tipo especial",
    }
}
