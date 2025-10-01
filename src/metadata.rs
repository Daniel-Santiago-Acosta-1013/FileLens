use crate::advanced_metadata::{
    extract_image_metadata, extract_office_metadata, extract_pdf_metadata,
};
use crate::directory::{EntryKind, count_directory_entries};
use crate::formatting::{format_optional_time, format_size};
use comfy_table::Color;
use console::style;
use infer::Infer;
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::Path;

const HASH_SIZE_LIMIT: u64 = 32 * 1024 * 1024; // 32 MiB

pub fn render_metadata(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "No se pudo obtener la metadata de `{}`: {error}",
            path.display()
        )
    })?;

    println!();

    print_property("Ruta ingresada", &path.display().to_string(), Color::White);

    let canonical = fs::canonicalize(path)
        .map(|real_path| real_path.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    print_property("Ruta resuelta", &canonical, Color::White);

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

    let kind = EntryKind::from(&metadata);
    print_property("Tipo", kind_label(&kind), Color::White);

    let size_str = match kind {
        EntryKind::File => format_size(metadata.len()),
        _ => format!("{} bytes", metadata.len()),
    };
    print_property("Tamaño", &size_str, Color::White);

    if matches!(kind, EntryKind::Directory)
        && let Ok((count, truncated)) = count_directory_entries(path)
    {
        let label = if truncated {
            format!("{count}+ elementos directos")
        } else {
            format!("{count} elementos directos")
        };
        print_property("Contenido", &label, Color::White);
    }

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
            &format_unix_permissions(metadata.permissions().mode()),
            Color::White,
        );

        let owner = owner_name(&metadata).unwrap_or_else(|| "Desconocido".to_string());
        print_property("Propietario", &owner, Color::White);

        let group = group_name(&metadata).unwrap_or_else(|| "Desconocido".to_string());
        print_property("Grupo", &group, Color::White);
    }

    if matches!(kind, EntryKind::File) {
        if let Some(mime) = mime_type(path) {
            print_property("Tipo MIME", &mime, Color::White);
        }

        print_property("Hash SHA-256", &file_hash(path, &metadata), Color::White);
    }

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

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path)
            .map(|t| t.display().to_string())
            .unwrap_or_else(|_| "No disponible".to_string());
        print_property("Enlace simbólico a", &target, Color::White);
    }

    // Metadata avanzada según tipo de archivo
    let kind = EntryKind::from(&metadata);
    if matches!(kind, EntryKind::File)
        && let Some(mime) = mime_type(path)
    {
        // Imágenes con EXIF
        if mime.starts_with("image/") {
            println!(
                "\n\n{}",
                style("━━━ Metadata EXIF ━━━").cyan().bold()
            );
            if extract_image_metadata(path) {
                println!(
                    "\n{}",
                    style("  ⚠  Esta imagen contiene metadata que puede revelar información sensible")
                        .yellow()
                );
            } else {
                println!(
                    "\n{}",
                    style("  No se encontró metadata EXIF en esta imagen").dim()
                );
            }
        }

        // PDFs
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

        // Documentos Office
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

    Ok(())
}

fn print_property(label: &str, value: &str, color: Color) {
    let label_styled = style(format!("  {}", label))
        .cyan()
        .bold();

    let arrow = style("→").dim();

    let value_styled = match color {
        Color::Yellow => style(value).yellow(),
        Color::Green => style(value).green(),
        Color::Red => style(value).red(),
        _ => style(value).white(),
    };

    println!("{} {} {}", label_styled, arrow, value_styled);
}

fn file_hash(path: &Path, metadata: &Metadata) -> String {
    if !metadata.is_file() {
        return "No aplica".to_string();
    }

    if metadata.len() > HASH_SIZE_LIMIT {
        return format!("Omitido (> {} MiB)", HASH_SIZE_LIMIT / (1024 * 1024));
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return format!("No disponible ({error})"),
    };

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => hasher.update(&buffer[..bytes_read]),
            Err(error) => return format!("No disponible ({error})"),
        }
    }

    let digest = hasher.finalize();
    format!("{:x}", digest)
}

fn mime_type(path: &Path) -> Option<String> {
    let infer = Infer::new();
    infer
        .get_from_path(path)
        .ok()
        .flatten()
        .map(|kind| kind.mime_type().to_string())
}

fn kind_label(kind: &EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "Directorio",
        EntryKind::File => "Archivo",
        EntryKind::Symlink => "Enlace simbólico",
        EntryKind::Other => "Tipo especial",
    }
}

#[cfg(unix)]
fn owner_name(metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    use users::get_user_by_uid;

    let uid = metadata.uid();
    get_user_by_uid(uid).map(|user| user.name().to_string_lossy().into_owned())
}

#[cfg(unix)]
fn group_name(metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    use users::get_group_by_gid;

    let gid = metadata.gid();
    get_group_by_gid(gid).map(|group| group.name().to_string_lossy().into_owned())
}

#[cfg(unix)]
fn format_unix_permissions(mode: u32) -> String {
    const SYMBOLS: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];

    let user = SYMBOLS[((mode >> 6) & 0o7) as usize];
    let group = SYMBOLS[((mode >> 3) & 0o7) as usize];
    let other = SYMBOLS[(mode & 0o7) as usize];

    format!("{}{}{}", user, group, other)
}
