//! Reúne y muestra la metadata del archivo solicitado.

use crate::advanced_metadata::{
    extract_image_metadata, extract_office_metadata, extract_pdf_metadata,
};
use crate::directory::{EntryKind, count_directory_entries};
use crate::formatting::{format_optional_time, format_size};
use comfy_table::Color;
use console::style;
use std::fs;
use std::path::Path;

use super::hashing::file_hash;
use super::mime::mime_type;
use super::output::print_property;

pub fn render_metadata(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "No se pudo obtener la metadata de `{}`: {error}",
            path.display()
        )
    })?;

    println!();

    render_path_details(path);
    render_name_details(path);

    let kind = EntryKind::from(&metadata);
    render_kind_details(&metadata, &kind);
    render_directory_summary(path, &kind);
    render_permissions(&metadata);
    render_file_specifics(path, &metadata, &kind);
    render_timestamps(&metadata);
    render_symlink_target(path, &metadata);

    render_advanced_metadata(path, &kind);

    Ok(())
}

fn render_path_details(path: &Path) {
    print_property("Ruta ingresada", &path.display().to_string(), Color::White);

    let canonical = fs::canonicalize(path)
        .map(|real_path| real_path.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    print_property("Ruta resuelta", &canonical, Color::White);
}

fn render_name_details(path: &Path) {
    if let Some(name) = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
    {
        print_property("Nombre", &name, Color::White);
    }

    if let Some(ext) = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
    {
        print_property("Extensión", &ext, Color::White);
    }
}

fn render_kind_details(metadata: &fs::Metadata, kind: &EntryKind) {
    print_property("Tipo", kind_label(kind), Color::White);

    let size_str = match kind {
        EntryKind::File => format_size(metadata.len()),
        _ => format!("{} bytes", metadata.len()),
    };
    print_property("Tamaño", &size_str, Color::White);
}

fn render_directory_summary(path: &Path, kind: &EntryKind) {
    if !matches!(kind, EntryKind::Directory) {
        return;
    }

    if let Ok((count, truncated)) = count_directory_entries(path) {
        let label = if truncated {
            format!("{count}+ elementos directos")
        } else {
            format!("{count} elementos directos")
        };
        print_property("Contenido", &label, Color::White);
    }
}

fn render_permissions(metadata: &fs::Metadata) {
    let readonly_color = if metadata.permissions().readonly() {
        Color::Yellow
    } else {
        Color::Green
    };
    let readonly_value = if metadata.permissions().readonly() {
        "Solo lectura"
    } else {
        "Lectura y escritura"
    };
    print_property("Permisos", readonly_value, readonly_color);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        print_property(
            "Permisos (octal)",
            &format!("{:04o}", metadata.permissions().mode() & 0o7777),
            Color::White,
        );
        print_property(
            "Permisos (rwx)",
            &super::permissions::format_unix_permissions(metadata.permissions().mode()),
            Color::White,
        );

        let owner =
            super::permissions::owner_name(metadata).unwrap_or_else(|| "Desconocido".to_string());
        print_property("Propietario", &owner, Color::White);

        let group =
            super::permissions::group_name(metadata).unwrap_or_else(|| "Desconocido".to_string());
        print_property("Grupo", &group, Color::White);
    }
}

fn render_file_specifics(path: &Path, metadata: &fs::Metadata, kind: &EntryKind) {
    if !matches!(kind, EntryKind::File) {
        return;
    }

    if let Some(mime) = mime_type(path) {
        print_property("Tipo MIME", &mime, Color::White);
    }

    print_property("Hash SHA-256", &file_hash(path, metadata), Color::White);
}

fn render_timestamps(metadata: &fs::Metadata) {
    print_property(
        "Último acceso",
        &format_optional_time(metadata.accessed().ok()),
        Color::White,
    );
    print_property(
        "Última modificación",
        &format_optional_time(metadata.modified().ok()),
        Color::White,
    );
    print_property(
        "Fecha de creación",
        &format_optional_time(metadata.created().ok()),
        Color::White,
    );
}

fn render_symlink_target(path: &Path, metadata: &fs::Metadata) {
    if !metadata.file_type().is_symlink() {
        return;
    }

    let target = fs::read_link(path)
        .map(|t| t.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    print_property("Enlace simbólico a", &target, Color::White);
}

fn render_advanced_metadata(path: &Path, kind: &EntryKind) {
    if !matches!(kind, EntryKind::File) {
        return;
    }

    if let Some(mime) = mime_type(path) {
        if mime.starts_with("image/") {
            println!("\n\n{}", style("━━━ Metadata EXIF ━━━").cyan().bold());
            if extract_image_metadata(path) {
                println!(
                    "\n{}",
                    style(
                        "  ⚠  Esta imagen contiene metadata que puede revelar información sensible"
                    )
                    .yellow()
                );
            } else {
                println!(
                    "\n{}",
                    style("  No se encontró metadata EXIF en esta imagen").dim()
                );
            }
        }

        if mime == "application/pdf" {
            println!("\n\n{}", style("━━━ Metadata PDF ━━━").cyan().bold());
            if extract_pdf_metadata(path) {
                println!(
                    "\n{}",
                    style("  ⚠  Este PDF contiene metadata que puede revelar información del autor y organización")
                        .yellow()
                );
            } else {
                println!(
                    "\n{}",
                    style("  No se encontró metadata adicional en este PDF").dim()
                );
            }
        }

        if mime.contains("officedocument")
            || mime.contains("msword")
            || mime.contains("ms-excel")
            || mime.contains("ms-powerpoint")
        {
            println!("\n\n{}", style("━━━ Metadata Office ━━━").cyan().bold());
            if extract_office_metadata(path) {
                println!(
                    "\n{}",
                    style("  ⚠  Este documento contiene metadata que puede revelar información personal y organizacional")
                        .yellow()
                );
            } else {
                println!(
                    "\n{}",
                    style("  No se pudo extraer metadata de este documento Office").dim()
                );
            }
        }
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
