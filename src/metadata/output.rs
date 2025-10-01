//! Utilidades de presentación para formatear propiedades en consola.

use comfy_table::Color;
use console::style;

/// Imprime una propiedad con el estilo consistente de FileLens.
pub fn print_property(label: &str, value: &str, color: Color) {
    let label_styled = style(format!("  {}", label)).cyan().bold();
    let arrow = style("→").dim();

    let value_styled = match color {
        Color::Yellow => style(value).yellow(),
        Color::Green => style(value).green(),
        Color::Red => style(value).red(),
        _ => style(value).white(),
    };

    println!("{} {} {}", label_styled, arrow, value_styled);
}
