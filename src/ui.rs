use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use console::style;
use std::path::Path;

const HEADER_WIDTH: usize = 70;

pub fn render_header() {
    let border = "═".repeat(HEADER_WIDTH - 2);
    println!("{}", style(format!("╔{}╗", border)).cyan().bold());
    println!(
        "{}",
        style(format!(
            "║ {:^inner_width$} ║",
            "FileLens | Inspector interactivo",
            inner_width = HEADER_WIDTH - 4
        ))
        .cyan()
        .bold()
    );
    println!("{}\n", style(format!("╚{}╝", border)).cyan().bold());
}

pub fn render_intro(current_dir: &Path) {
    println!(
        "{}",
        style("Explora la metadata de cualquier recurso del sistema de archivos.").dim()
    );
    println!(
        "{}",
        style(format!("Comienza desde: {}", current_dir.display())).dim()
    );
    render_help();
}

pub fn render_help() {
    println!(
        "{}",
        style("Comandos: `ls` lista, `cd <ruta|#>` navega, `ver <ruta|#>` muestra metadata, `..` retrocede, `salir` termina.")
            .dim()
    );
    println!(
        "{}\n",
        style("También puedes escribir directamente una ruta o un número listado para ver su metadata.")
            .dim()
    );
}

pub fn render_navigation_hint() {
    println!(
        "{}\n",
        style("Ayuda rápida: `ls` refresca la carpeta actual, `cd` acepta rutas o números, `ver` muestra detalles. Usa `ayuda` para repetir las instrucciones.")
            .dim()
    );
}

pub fn base_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

pub fn header_cell(text: &str) -> Cell {
    Cell::new(text)
        .fg(Color::Cyan)
        .add_attribute(Attribute::Bold)
        .add_attribute(Attribute::Underlined)
}

pub fn label_cell(code: &str, label: &str) -> Cell {
    Cell::new(format!("[{code}] {label}")).fg(Color::Rgb {
        r: 160,
        g: 196,
        b: 255,
    })
}
