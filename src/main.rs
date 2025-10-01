mod advanced_metadata;
mod directory;
mod formatting;
mod metadata;
mod search;
mod ui;

use console::{Term, style};
use std::env;
use std::io::{self, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        eprintln!("FileLens es interactivo y no acepta argumentos.");
        std::process::exit(1);
    }

    let term = Term::stdout();

    loop {
        term.clear_screen().ok();
        ui::render_header();

        print!("{}", style("│ Nombre del archivo ▸ ").cyan());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() {
            println!(
                "\n{}",
                style("│ Error: Debes ingresar un nombre de archivo.").red()
            );
            if !ask_continue() {
                break;
            }
            continue;
        }

        let path = Path::new(input);

        if path.exists() {
            show_metadata(path);
        } else {
            println!();
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
            } else if matches.len() == 1 {
                show_metadata(&matches[0]);
            } else {
                println!(
                    "\n{}",
                    style(format!(
                        "┌─ Se encontraron {} coincidencias para '{}'",
                        matches.len(),
                        input
                    ))
                    .yellow()
                );
                for (index, path) in matches.iter().enumerate() {
                    println!(
                        "{}",
                        style(format!("│ [{}] {}", index + 1, path.display())).dim()
                    );
                }
                println!("{}", style("└─").yellow());

                print!(
                    "\n{}",
                    style(format!("│ Selecciona [1-{}] ▸ ", matches.len())).cyan()
                );
                io::stdout().flush().unwrap();

                let mut selection = String::new();
                io::stdin().read_line(&mut selection).unwrap();

                let selected_index: usize = match selection.trim().parse::<usize>() {
                    Ok(num) if num >= 1 && num <= matches.len() => num - 1,
                    _ => {
                        println!("\n{}", style("│ Selección inválida.").red());
                        if !ask_continue() {
                            break;
                        }
                        continue;
                    }
                };

                show_metadata(&matches[selected_index]);
            }
        }

        if !ask_continue() {
            break;
        }
    }

    println!(
        "\n{}",
        style("│ Gracias por usar FileLens. ¡Hasta pronto!").cyan()
    );
}

fn show_metadata(path: &Path) {
    println!();
    if let Err(error) = metadata::render_metadata(path) {
        println!("\n{}", style(format!("│ Error: {}", error)).red());
    }
}

fn ask_continue() -> bool {
    println!();
    print!("{}", style("│ ¿Analizar otro archivo? (s/n) ▸ ").cyan());
    io::stdout().flush().unwrap();

    let mut response = String::new();
    io::stdin().read_line(&mut response).unwrap();
    let response = response.trim().to_lowercase();

    matches!(response.as_str(), "s" | "si" | "y" | "yes" | "")
}
