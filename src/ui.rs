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
