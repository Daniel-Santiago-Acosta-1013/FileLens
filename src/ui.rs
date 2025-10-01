use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use console::style;

const HEADER_WIDTH: usize = 74;

pub fn render_header() {
    let border = "─".repeat(HEADER_WIDTH - 2);
    println!("\n{}", style(format!("┌{}┐", border)).cyan());
    println!(
        "{}",
        style(format!(
            "│ {:^inner_width$} │",
            "▸ FileLens · Analizador de Metadata de Archivos ◂",
            inner_width = HEADER_WIDTH - 4
        ))
        .cyan()
        .bold()
    );
    println!("{}\n", style(format!("└{}┘", border)).cyan());
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
