use chrono::{DateTime, Local};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Row, Table};
use console::style;
use std::env;
use std::fs::{self, Metadata};
use std::io::{self, Write};
use std::path::Path;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn main() {
    if env::args().len() > 1 {
        eprintln!(
            "FileLens es interactivo y no acepta argumentos. Ejecuta solo `cargo run` o el binario sin parámetros."
        );
        std::process::exit(1);
    }

    render_header();

    println!(
        "{}",
        style("Escribe la ruta de un archivo o directorio para ver su metadata.").dim()
    );
    println!(
        "{}\n",
        style("Escribe 'salir' o 'exit' para terminar.").dim()
    );

    let mut input = String::new();
    loop {
        match read_user_input(&mut input) {
            Ok(None) => {
                println!("\n{}", style("Fin de la entrada. ¡Hasta luego!").dim());
                break;
            }
            Ok(Some(line)) => {
                if line.eq_ignore_ascii_case("exit") || line.eq_ignore_ascii_case("salir") {
                    println!("{}", style("Hasta luego!").dim());
                    break;
                }

                if line.is_empty() {
                    continue;
                }

                match show_metadata(&line) {
                    Ok(()) => println!("{}\n", style("Consulta completada.").dim()),
                    Err(message) => eprintln!("{message}"),
                }
            }
            Err(error) => {
                eprintln!("Error al leer la entrada: {error}");
            }
        }
    }
}

fn render_header() {
    let width = 66;
    let border = "═".repeat(width - 2);
    println!("{}", style(format!("╔{}╗", border)).cyan().bold());
    println!(
        "{}",
        style(format!(
            "║ {:^inner_width$} ║",
            "FileLens | Inspector de archivos",
            inner_width = width - 4
        ))
        .cyan()
        .bold()
    );
    println!("{}\n", style(format!("╚{}╝", border)).cyan().bold());
}

fn read_user_input(buffer: &mut String) -> io::Result<Option<String>> {
    print!("{} ", style("Ruta").bold().cyan());
    print!("{} ", style("›").cyan());
    io::stdout().flush()?;

    buffer.clear();
    let bytes_read = io::stdin().read_line(buffer)?;
    if bytes_read == 0 {
        return Ok(None);
    }

    Ok(Some(buffer.trim().to_string()))
}

fn show_metadata(path_str: &str) -> Result<(), String> {
    let path = Path::new(path_str);

    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("No se pudo obtener la metadata de `{}`: {error}", path_str))?;

    let mut table = build_table();

    table.add_row(build_row(
        "Ruta proporcionada",
        &path.display().to_string(),
        Color::White,
    ));

    let canonical = fs::canonicalize(path)
        .map(|real_path| real_path.display().to_string())
        .unwrap_or_else(|_| "No disponible".to_string());
    table.add_row(build_row("Ruta real", &canonical, Color::White));

    table.add_row(build_row(
        "Tipo",
        describe_file_type(&metadata),
        Color::White,
    ));
    table.add_row(build_row(
        "Tamaño",
        &format!("{} bytes", metadata.len()),
        Color::White,
    ));

    let readonly_color = if metadata.permissions().readonly() {
        Color::Yellow
    } else {
        Color::Green
    };
    let readonly_value = if metadata.permissions().readonly() {
        "Sí"
    } else {
        "No"
    };
    table.add_row(build_row("Solo lectura", readonly_value, readonly_color));

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode();
        table.add_row(build_row(
            "Permisos (octal)",
            &format!("{:04o}", mode & 0o7777),
            Color::White,
        ));
        table.add_row(build_row(
            "Permisos (rwx)",
            &format_unix_permissions(mode),
            Color::White,
        ));
    }

    table.add_row(build_row(
        "Último acceso",
        &format_optional_time(metadata.accessed().ok()),
        Color::White,
    ));
    table.add_row(build_row(
        "Última modificación",
        &format_optional_time(metadata.modified().ok()),
        Color::White,
    ));
    table.add_row(build_row(
        "Creación",
        &format_optional_time(metadata.created().ok()),
        Color::White,
    ));

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path)
            .map(|t| t.display().to_string())
            .unwrap_or_else(|_| "No disponible".to_string());
        table.add_row(build_row("Apunta a", &target, Color::White));
    }

    println!("\n{table}");
    Ok(())
}

fn build_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![header_cell("Propiedad"), header_cell("Valor")]);

    table
}

fn header_cell(text: &str) -> Cell {
    Cell::new(text)
        .fg(Color::Cyan)
        .add_attribute(Attribute::Bold)
        .add_attribute(Attribute::Underlined)
}

fn build_row(label: &str, value: &str, value_color: Color) -> Row {
    Row::from(vec![
        Cell::new(label).fg(Color::Rgb {
            r: 160,
            g: 196,
            b: 255,
        }),
        Cell::new(value).fg(value_color),
    ])
}

fn format_optional_time(time: Option<SystemTime>) -> String {
    match time {
        Some(value) => format_system_time(value),
        None => "No disponible".to_string(),
    }
}

fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn describe_file_type(metadata: &Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_file() {
        "Archivo"
    } else if file_type.is_dir() {
        "Directorio"
    } else if file_type.is_symlink() {
        "Enlace simbólico"
    } else {
        "Tipo especial"
    }
}

#[cfg(unix)]
fn format_unix_permissions(mode: u32) -> String {
    const SYMBOLS: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];

    let user = SYMBOLS[((mode >> 6) & 0o7) as usize];
    let group = SYMBOLS[((mode >> 3) & 0o7) as usize];
    let other = SYMBOLS[(mode & 0o7) as usize];

    format!("{}{}{}", user, group, other)
}
