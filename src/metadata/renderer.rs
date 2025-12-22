//! Reúne la metadata del archivo solicitado.

use crate::advanced_metadata::{extract_image_metadata, extract_office_metadata, extract_pdf_metadata};
use crate::directory::{count_directory_entries, EntryKind};
use crate::formatting::{format_optional_time, format_size};
use std::fs;
use std::path::Path;

use super::hashing::file_hash;
use super::mime::mime_type;
use super::report::{MetadataOptions, MetadataReport, ReportEntry, ReportSection};

pub fn build_report(path: &Path, options: &MetadataOptions) -> Result<MetadataReport, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "No se pudo obtener la metadata de `{}`: {error}",
            path.display()
        )
    })?;

    let kind = EntryKind::from(&metadata);
    let mime = if matches!(kind, EntryKind::File) {
        mime_type(path)
    } else {
        None
    };

    let mut report = MetadataReport::new();
    report.system.extend(collect_path_details(path));
    report.system.extend(collect_name_details(path));
    report.system.extend(collect_kind_details(&metadata, &kind));

    if let Some(entry) = collect_directory_summary(path, &kind) {
        report.system.push(entry);
    }

    report.system.extend(collect_permissions(&metadata));
    report.system
        .extend(collect_file_specifics(path, &metadata, &kind, mime.as_deref(), options));
    report.system.extend(collect_timestamps(&metadata));

    if let Some(entry) = collect_symlink_target(path, &metadata) {
        report.system.push(entry);
    }

    let (sections, risks) = collect_advanced_metadata(path, &kind, mime.as_deref());
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
    options: &MetadataOptions,
) -> Vec<ReportEntry> {
    if !matches!(kind, EntryKind::File) {
        return Vec::new();
    }

    let mut entries = Vec::new();

    if let Some(mime) = mime {
        entries.push(ReportEntry::info("Tipo MIME", mime));
    }

    let hash_value = if options.include_hash {
        file_hash(path, metadata)
    } else {
        "Omitido (desactivado)".to_string()
    };
    entries.push(ReportEntry::info("Hash SHA-256", hash_value));

    entries
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
) -> (Vec<ReportSection>, Vec<ReportEntry>) {
    if !matches!(kind, EntryKind::File) {
        return (Vec::new(), Vec::new());
    }

    let mut sections = Vec::new();
    let mut risks = Vec::new();

    if let Some(mime) = mime {
        if mime.starts_with("image/") {
            let result = extract_image_metadata(path);
            sections.push(result.section);
            risks.extend(result.risks);
        }

        if mime == "application/pdf" {
            let result = extract_pdf_metadata(path);
            sections.push(result.section);
            risks.extend(result.risks);
        }

        if mime.contains("officedocument")
            || mime.contains("msword")
            || mime.contains("ms-excel")
            || mime.contains("ms-powerpoint")
        {
            let result = extract_office_metadata(path);
            sections.push(result.section);
            risks.extend(result.risks);
        }
    }

    (sections, risks)
}

fn kind_label(kind: &EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "Directorio",
        EntryKind::File => "Archivo",
        EntryKind::Symlink => "Enlace simbólico",
        EntryKind::Other => "Tipo especial",
    }
}
