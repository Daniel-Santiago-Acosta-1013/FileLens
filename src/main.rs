mod advanced_metadata;
mod directory;
mod formatting;
mod metadata;
mod metadata_editor;
mod search;
mod ui;

use console::{Term, style};
use rustyline::{DefaultEditor, error::ReadlineError};
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
    let mut file_input_editor =
        DefaultEditor::new().expect("No se pudo inicializar el editor de entrada");
    let mut directory_input_editor =
        DefaultEditor::new().expect("No se pudo inicializar el editor de entrada");

    loop {
        term.clear_screen().ok();
        ui::render_header();
        match ui::prompt_main_action() {
            ui::MainAction::AnalyzeFile => {
                if !handle_file_mode(&term, &mut file_input_editor) {
                    break;
                }
            }
            ui::MainAction::CleanDirectory => {
                if !handle_directory_mode(&term, &mut directory_input_editor) {
                    break;
                }
            }
            ui::MainAction::Exit => break,
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
        return;
    }

    // Preguntar si desea editar/eliminar metadata
    println!();
    print!(
        "{}",
        style("│ ¿Deseas editar o eliminar metadata? (s/n) ▸ ").cyan()
    );
    io::stdout().flush().unwrap();

    let mut response = String::new();
    io::stdin().read_line(&mut response).unwrap();
    let response = response.trim().to_lowercase();

    if matches!(response.as_str(), "s" | "si" | "y" | "yes")
        && let Err(error) = metadata_editor::show_edit_menu(path)
    {
        println!("\n{}", style(format!("│ Error: {}", error)).red());
    }
}

fn handle_file_mode(term: &Term, editor: &mut DefaultEditor) -> bool {
    loop {
        term.clear_screen().ok();
        ui::render_header();
        ui::render_file_mode_hint();

        let prompt = format!("{} ", style("│ Ruta o nombre del archivo ▸").cyan());
        let Some(input) = read_line_with_history(editor, &prompt) else {
            return true;
        };

        if input.is_empty() {
            println!(
                "\n{}",
                style("│ Error: Debes ingresar un nombre o ruta de archivo.").red()
            );
            continue;
        }

        let path = Path::new(&input);

        if path.exists() {
            show_metadata(path);
        } else {
            println!();
            let matches = search::find_files(&input);

            if matches.is_empty() {
                println!("\n{}", style("┌─ No se encontraron coincidencias").red());
                println!("{}", style(format!("│ No existe '{}' en:", input)).red());
                println!("{}", style("│   • Directorio actual").red());
                println!("{}", style("│   • ~/Documents").red());
                println!("{}", style("│   • ~/Downloads").red());
                println!("{}", style("│   • ~/Desktop").red());
                println!("{}", style("│   • ~/ (Home)").red());
                println!("{}", style("└─").red());
                continue;
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
                        continue;
                    }
                };

                show_metadata(&matches[selected_index]);
            }
        }

        if !ask_again("│ ¿Analizar otro archivo? (s/n) ▸ ") {
            return true;
        }
    }
}

fn handle_directory_mode(term: &Term, editor: &mut DefaultEditor) -> bool {
    loop {
        term.clear_screen().ok();
        ui::render_header();
        ui::render_directory_mode_hint();

        let prompt = format!("{} ", style("│ Ruta del directorio ▸").cyan());
        let Some(input) = read_line_with_history(editor, &prompt) else {
            return true;
        };

        if input.is_empty() {
            println!(
                "\n{}",
                style("│ Error: Debes ingresar la ruta de un directorio.").red()
            );
            continue;
        }

        let path = Path::new(&input);

        if path.is_dir() {
            println!();
            match metadata::render_metadata(path) {
                Ok(()) => {
                    if let Err(error) = metadata_editor::run_directory_cleanup(path) {
                        println!("\n{}", style(format!("│ Error: {}", error)).red());
                    }
                }
                Err(error) => {
                    println!("\n{}", style(format!("│ Error: {}", error)).red());
                }
            }
        } else if path.exists() {
            println!(
                "\n{}",
                style("│ La ruta indicada corresponde a un archivo, no a un directorio.").red()
            );
        } else {
            println!();
            let matches = search::find_directories(&input);

            if matches.is_empty() {
                println!("\n{}", style("┌─ No se encontraron directorios").red());
                println!(
                    "{}",
                    style(format!(
                        "│ No existe '{}' en los destinos habituales.",
                        input
                    ))
                    .red()
                );
                println!("{}", style("└─").red());
                continue;
            } else if matches.len() == 1 {
                let dir = &matches[0];
                println!();
                match metadata::render_metadata(dir) {
                    Ok(()) => {
                        if let Err(error) = metadata_editor::run_directory_cleanup(dir) {
                            println!("\n{}", style(format!("│ Error: {}", error)).red());
                        }
                    }
                    Err(error) => {
                        println!("\n{}", style(format!("│ Error: {}", error)).red());
                    }
                }
            } else {
                println!(
                    "\n{}",
                    style(format!(
                        "┌─ Se encontraron {} directorios para '{}'",
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
                        continue;
                    }
                };

                let dir = &matches[selected_index];
                println!();
                match metadata::render_metadata(dir) {
                    Ok(()) => {
                        if let Err(error) = metadata_editor::run_directory_cleanup(dir) {
                            println!("\n{}", style(format!("│ Error: {}", error)).red());
                        }
                    }
                    Err(error) => {
                        println!("\n{}", style(format!("│ Error: {}", error)).red());
                    }
                }
            }
        }

        if !ask_again("│ ¿Limpiar otro directorio? (s/n) ▸ ") {
            return true;
        }
    }
}

fn read_line_with_history(editor: &mut DefaultEditor, prompt: &str) -> Option<String> {
    match editor.readline(prompt) {
        Ok(line) => {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                let _ = editor.add_history_entry(trimmed.as_str());
            }
            Some(trimmed)
        }
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => None,
        Err(err) => {
            println!(
                "\n{}",
                style(format!("│ Error leyendo la entrada: {}", err)).red()
            );
            None
        }
    }
}

fn ask_again(prompt: &str) -> bool {
    println!();
    print!("{}", style(prompt).cyan());
    io::stdout().flush().unwrap();

    let mut response = String::new();
    io::stdin().read_line(&mut response).unwrap();
    let response = response.trim().to_lowercase();

    matches!(response.as_str(), "s" | "si" | "y" | "yes" | "")
}
