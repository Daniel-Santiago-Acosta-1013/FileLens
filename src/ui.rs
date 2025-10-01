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

pub fn render_file_input_hint() {
    let hint_lines = [
        "┌─ Puedes ingresar:",
        "│   • Un nombre con extensión (ej. reporte.pdf)",
        "│   • Una ruta relativa (ej. ./docs/reporte.pdf)",
        "│   • Una ruta absoluta (ej. /Users/usuario/reporte.pdf)",
        "└─",
    ];

    for line in hint_lines.iter() {
        println!("{}", style(line).cyan().dim());
    }

    println!();
}
