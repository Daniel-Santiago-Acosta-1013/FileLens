use crate::directory::{EntryKind, count_directory_entries};
use crate::formatting::{format_optional_time, format_size};
use crate::ui::{base_table, header_cell, label_cell};
use comfy_table::{Cell, Color, Row};
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

    let mut table = base_table();
    table.set_header(vec![header_cell("Propiedad"), header_cell("Valor")]);

    table.add_row(property_row(
        "RUTA",
        "Ruta ingresada",
        path.display().to_string(),
        Color::White,
    ));

    let canonical = fs::canonicalize(path)
        .map(|real_path| real_path.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    table.add_row(property_row(
        "REAL",
        "Ruta resuelta",
        canonical,
        Color::White,
    ));

    if let Some(name) = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
    {
        table.add_row(property_row("NOMB", "Nombre", name, Color::White));
    }

    if let Some(ext) = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
    {
        table.add_row(property_row("EXT", "Extensión", ext, Color::White));
    }

    let kind = EntryKind::from(&metadata);
    table.add_row(property_row(
        "TIPO",
        "Tipo",
        kind_label(&kind),
        Color::White,
    ));

    table.add_row(property_row(
        "TAM",
        "Tamaño",
        match kind {
            EntryKind::File => format_size(metadata.len()),
            _ => format!("{} bytes", metadata.len()),
        },
        Color::White,
    ));

    if matches!(kind, EntryKind::Directory)
        && let Ok((count, truncated)) = count_directory_entries(path)
    {
        let label = if truncated {
            format!("{count}+ elementos directos")
        } else {
            format!("{count} elementos directos")
        };
        table.add_row(property_row("CONT", "Contenido", label, Color::White));
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
    table.add_row(property_row(
        "ACCE",
        "Accesos",
        readonly_value,
        readonly_color,
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        table.add_row(property_row(
            "PERM",
            "Permisos (octal)",
            format!("{:04o}", metadata.permissions().mode() & 0o7777),
            Color::White,
        ));
        table.add_row(property_row(
            "PERM",
            "Permisos (rwx)",
            format_unix_permissions(metadata.permissions().mode()),
            Color::White,
        ));

        let owner = owner_name(&metadata).unwrap_or_else(|| "Desconocido".to_string());
        table.add_row(property_row("USR", "Propietario", owner, Color::White));

        let group = group_name(&metadata).unwrap_or_else(|| "Desconocido".to_string());
        table.add_row(property_row("GRP", "Grupo", group, Color::White));
    }

    if matches!(kind, EntryKind::File) {
        if let Some(mime) = mime_type(path) {
            table.add_row(property_row(
                "MIME",
                "Tipo de contenido",
                mime,
                Color::White,
            ));
        }

        table.add_row(property_row(
            "HASH",
            "SHA-256",
            file_hash(path, &metadata),
            Color::White,
        ));
    }

    table.add_row(property_row(
        "ATIM",
        "Último acceso",
        format_optional_time(metadata.accessed().ok()),
        Color::White,
    ));
    table.add_row(property_row(
        "MTIM",
        "Última modificación",
        format_optional_time(metadata.modified().ok()),
        Color::White,
    ));
    table.add_row(property_row(
        "CTIM",
        "Creación",
        format_optional_time(metadata.created().ok()),
        Color::White,
    ));

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path)
            .map(|t| t.display().to_string())
            .unwrap_or_else(|_| "No disponible".to_string());
        table.add_row(property_row("LINK", "Enlace a", target, Color::White));
    }

    println!("\n{table}");
    Ok(())
}

fn property_row(code: &str, label: &str, value: impl Into<String>, color: Color) -> Row {
    Row::from(vec![
        label_cell(code, label),
        Cell::new(value.into()).fg(color),
    ])
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
