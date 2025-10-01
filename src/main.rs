mod directory;
mod formatting;
mod metadata;
mod search;
mod ui;

use console::style;
use std::env;
use std::io::{self, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        eprintln!("FileLens es interactivo y no acepta argumentos.");
        std::process::exit(1);
    }

    ui::render_header();

    print!("{}", style("│ Nombre del archivo ▸ ").cyan());
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim();

    if input.is_empty() {
        eprintln!("Error: Debes ingresar un nombre de archivo.");
        std::process::exit(1);
    }

    let path = Path::new(input);

    if path.exists() {
        show_metadata(path);
        return;
    }

    println!("\n{}", style("│ Buscando archivo en el sistema...").dim());
    let matches = search::find_files(input);

    if matches.is_empty() {
        println!("\n{}", style("┌─ No se encontraron coincidencias").red());
        println!("{}", style(format!("│ No existe '{}' en:", input)).red());
        println!("{}", style("│   • Directorio actual").red());
        println!("{}", style("│   • ~/Documents").red());
        println!("{}", style("│   • ~/Downloads").red());
        println!("{}", style("│   • ~/Desktop").red());
        println!("{}", style("│   • ~/ (Home)").red());
        println!("{}", style("└─").red());
        std::process::exit(1);
    }

    if matches.len() == 1 {
        show_metadata(&matches[0]);
        return;
    }

    println!("\n{}", style(format!("┌─ Se encontraron {} coincidencias para '{}'", matches.len(), input)).yellow());
    for (index, path) in matches.iter().enumerate() {
        println!("{}", style(format!("│ [{}] {}", index + 1, path.display())).dim());
    }
    println!("{}", style("└─").yellow());

    print!("\n{}", style(format!("│ Selecciona [1-{}] ▸ ", matches.len())).cyan());
    io::stdout().flush().unwrap();

    let mut selection = String::new();
    io::stdin().read_line(&mut selection).unwrap();

    let selected_index: usize = match selection.trim().parse::<usize>() {
        Ok(num) if num >= 1 && num <= matches.len() => num - 1,
        _ => {
            eprintln!("Selección inválida.");
            std::process::exit(1);
        }
    };

    show_metadata(&matches[selected_index]);
}

fn show_metadata(path: &Path) {
    println!();
    if let Err(error) = metadata::render_metadata(path) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
