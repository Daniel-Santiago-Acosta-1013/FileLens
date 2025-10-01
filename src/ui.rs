use console::style;
use std::io::{self, Write};

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

pub enum MainAction {
    AnalyzeFile,
    CleanDirectory,
    Exit,
}

pub fn prompt_main_action() -> MainAction {
    println!("{}", style("┌─ Menú Principal ─").cyan());
    println!("{}", style("│  [1] Analizar un archivo individual").cyan());
    println!("{}", style("│  [2] Limpieza masiva de directorio").cyan());
    println!("{}", style("│  [3] Salir").cyan());
    println!("{}", style("└─").cyan());

    loop {
        print!("{}", style("\n│ Selecciona una opción ▸ ").cyan());
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => return MainAction::AnalyzeFile,
            "2" => return MainAction::CleanDirectory,
            "3" => return MainAction::Exit,
            _ => println!(
                "{}",
                style("│ Opción inválida. Intenta nuevamente.").yellow()
            ),
        }
    }
}

pub fn render_file_mode_hint() {
    let hint_lines = [
        "┌─ Analizar archivo:",
        "│   • Ingresar un nombre con extensión (ej. reporte.pdf)",
        "│   • Usar rutas relativas o absolutas",
        "│   • Se mostrará la metadata y opciones de edición",
        "└─",
    ];

    for line in hint_lines.iter() {
        println!("{}", style(line).cyan().dim());
    }

    println!();
}

pub fn render_directory_mode_hint() {
    let hint_lines = [
        "┌─ Limpieza de directorio:",
        "│   • Ingresa la ruta del directorio base",
        "│   • Se puede decidir incluir subdirectorios",
        "│   • Elige qué tipos de archivos limpiar",
        "└─",
    ];

    for line in hint_lines.iter() {
        println!("{}", style(line).cyan().dim());
    }

    println!();
}
