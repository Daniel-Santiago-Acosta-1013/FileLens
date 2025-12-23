//! Extracción de metadata de archivos ZIP.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use std::fs::File;
use std::path::Path;

pub fn extract_zip_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata ZIP");
    let risks = Vec::new();

    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer el archivo ZIP",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo interpretar el contenido ZIP",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    section
        .entries
        .push(ReportEntry::info("Entradas", archive.len().to_string()));

    if let Ok(comment) = std::str::from_utf8(archive.comment()) {
        if !comment.trim().is_empty() {
            section
                .entries
                .push(ReportEntry::info("Comentario ZIP", comment.trim()));
        }
    }

    let zip64 = archive.zip64_comment().is_some();
    section
        .entries
        .push(ReportEntry::info("ZIP64", if zip64 { "Sí" } else { "No" }));

    let mut encrypted = false;
    let mut entry_details = Vec::new();
    for index in 0..archive.len() {
        if let Ok(file) = archive.by_index(index) {
            if file.encrypted() {
                encrypted = true;
            }
            if entry_details.len() < 50 {
                entry_details.push(format_zip_entry(index + 1, &file));
            }
        }
    }
    section.entries.push(ReportEntry::info(
        "Cifrado ZIP",
        if encrypted { "Sí" } else { "No" },
    ));

    if !entry_details.is_empty() {
        for entry in entry_details {
            section.entries.push(ReportEntry::info(entry.0, entry.1));
        }
        if archive.len() > 50 {
            section.entries.push(ReportEntry::new(
                "Entradas omitidas",
                format!("{}", archive.len() - 50),
                EntryLevel::Muted,
            ));
        }
    }

    AdvancedMetadataResult { section, risks }
}

fn format_zip_entry(index: usize, file: &zip::read::ZipFile) -> (String, String) {
    let name = file.name();
    let compression = format!("{:?}", file.compression());
    let crc32 = format!("{:08x}", file.crc32());
    let last_modified = file
        .last_modified()
        .map(|time| time.to_string())
        .unwrap_or_else(|| "N/D".to_string());
    let flags = format!(
        "encrypted:{} utf8:{}",
        yes_no(file.encrypted()),
        yes_no(file.name_raw().is_ascii())
    );
    let extra_len = file.extra_data().map(|data| data.len()).unwrap_or(0);
    let perm = file
        .unix_mode()
        .map(|mode| format!("{mode:o}"))
        .unwrap_or_else(|| "N/D".to_string());
    let is_dir = yes_no(file.is_dir());

    let label = format!("Entrada {index} · {name}");
    let value = format!(
        "comp:{} | tamaño:{} | sin comp:{} | crc32:{} | fecha:{} | flags:{} | extra:{} bytes | perm:{} | dir:{}",
        compression,
        file.compressed_size(),
        file.size(),
        crc32,
        last_modified,
        flags,
        extra_len,
        perm,
        is_dir
    );
    (label, value)
}

fn yes_no(value: bool) -> &'static str {
    if value { "Sí" } else { "No" }
}
