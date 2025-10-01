use crate::directory::{self, EntrySummary};
use crate::metadata;
use crate::ui;
use console::style;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run() -> Result<(), String> {
    let mut state =
        AppState::new().map_err(|error| format!("No se pudo inicializar FileLens: {error}"))?;

    ui::render_header();
    ui::render_intro(&state.current_dir);

    match refresh_listing(&mut state) {
        Ok(()) => ui::render_navigation_hint(),
        Err(message) => eprintln!("{message}"),
    }

    let mut input = String::new();
    loop {
        match read_user_input(&mut input, &state.current_dir) {
            Ok(None) => {
                println!("\n{}", style("Fin de la entrada. ¡Hasta luego!").dim());
                break;
            }
            Ok(Some(line)) => {
                if line.is_empty() {
                    continue;
                }

                if matches_command(&line, &["exit", "salir"]) {
                    println!("{}", style("Hasta luego!").dim());
                    break;
                }

                if matches_command(&line, &["ayuda", "help"]) {
                    ui::render_help();
                    continue;
                }

                match handle_input(&mut state, &line) {
                    Ok(ActionResult::None) => {}
                    Ok(ActionResult::Listed) | Ok(ActionResult::MetadataShown) => {
                        ui::render_navigation_hint()
                    }
                    Err(message) => eprintln!("{message}"),
                }
            }
            Err(error) => {
                eprintln!("Error al leer la entrada: {error}");
            }
        }
    }

    Ok(())
}

fn refresh_listing(state: &mut AppState) -> Result<(), String> {
    let entries = directory::read_directory(&state.current_dir)?;
    directory::render_directory_table(&entries, &state.current_dir)?;
    state.last_listing = entries;
    Ok(())
}

fn matches_command(input: &str, aliases: &[&str]) -> bool {
    aliases
        .iter()
        .any(|alias| input.eq_ignore_ascii_case(alias))
}

struct AppState {
    current_dir: PathBuf,
    last_listing: Vec<EntrySummary>,
}

impl AppState {
    fn new() -> io::Result<Self> {
        Ok(Self {
            current_dir: env::current_dir()?,
            last_listing: Vec::new(),
        })
    }

    fn resolve_path(&self, input: &str) -> PathBuf {
        let candidate = Path::new(input);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.current_dir.join(candidate)
        }
    }

    fn entry_by_index(&self, index: usize) -> Option<&EntrySummary> {
        self.last_listing.get(index)
    }
}

enum ActionResult {
    None,
    Listed,
    MetadataShown,
}

fn handle_input(state: &mut AppState, raw_input: &str) -> Result<ActionResult, String> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return Ok(ActionResult::None);
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or("");

    match command.to_ascii_lowercase().as_str() {
        "ls" | "dir" | "listar" => {
            refresh_listing(state)?;
            Ok(ActionResult::Listed)
        }
        "cd" => {
            let remainder = trimmed[command.len()..].trim();
            if remainder.is_empty() {
                return Err("Debes indicar la ruta o índice al que deseas ir.".to_string());
            }
            change_directory(state, remainder)?;
            refresh_listing(state)?;
            Ok(ActionResult::Listed)
        }
        "ver" | "info" => {
            let remainder = trimmed[command.len()..].trim();
            if remainder.is_empty() {
                return Err("Debes indicar la ruta o índice que deseas inspeccionar.".to_string());
            }
            show_metadata_from_input(state, remainder)?;
            Ok(ActionResult::MetadataShown)
        }
        ".." => {
            change_directory(state, "..")?;
            refresh_listing(state)?;
            Ok(ActionResult::Listed)
        }
        _ => {
            if let Ok(index) = trimmed.parse::<usize>() {
                let zero_based = index
                    .checked_sub(1)
                    .ok_or_else(|| "El índice debe ser positivo.".to_string())?;
                if let Some(entry) = state.entry_by_index(zero_based) {
                    metadata::render_metadata(&entry.path)?;
                    return Ok(ActionResult::MetadataShown);
                }
                return Err("Índice fuera de rango. Usa `ls` para actualizar la lista.".to_string());
            }

            show_metadata_from_input(state, trimmed)?;
            Ok(ActionResult::MetadataShown)
        }
    }
}

fn change_directory(state: &mut AppState, target: &str) -> Result<(), String> {
    let new_path = if let Ok(index) = target.parse::<usize>() {
        let zero_based = index
            .checked_sub(1)
            .ok_or_else(|| "El índice debe ser positivo.".to_string())?;
        state
            .entry_by_index(zero_based)
            .map(|entry| entry.path.clone())
            .ok_or_else(|| {
                "Índice fuera de rango. Usa `ls` para actualizar la lista.".to_string()
            })?
    } else {
        state.resolve_path(target)
    };

    let metadata = fs::symlink_metadata(&new_path)
        .map_err(|error| format!("No se pudo acceder a `{}`: {error}", new_path.display()))?;

    let destination = if metadata.is_dir() {
        new_path
    } else if metadata.is_symlink() {
        fs::canonicalize(&new_path).map_err(|error| {
            format!(
                "No se pudo resolver el enlace `{}`: {error}",
                new_path.display()
            )
        })?
    } else {
        return Err("Solo puedes navegar hacia directorios. Usa `ver` para archivos.".to_string());
    };

    state.current_dir = destination;
    Ok(())
}

fn show_metadata_from_input(state: &mut AppState, input: &str) -> Result<(), String> {
    if let Ok(index) = input.parse::<usize>() {
        let zero_based = index
            .checked_sub(1)
            .ok_or_else(|| "El índice debe ser positivo.".to_string())?;
        let entry = state.entry_by_index(zero_based).ok_or_else(|| {
            "Índice fuera de rango. Usa `ls` para actualizar la lista.".to_string()
        })?;
        metadata::render_metadata(&entry.path)
    } else {
        let path = state.resolve_path(input);
        metadata::render_metadata(&path)
    }
}

fn read_user_input(buffer: &mut String, current_dir: &Path) -> io::Result<Option<String>> {
    print!(
        "{} {} ",
        style("Ubicación").bold().cyan(),
        style(current_dir.display()).dim()
    );
    print!("{} ", style("›").cyan());
    io::stdout().flush()?;

    buffer.clear();
    let bytes_read = io::stdin().read_line(buffer)?;
    if bytes_read == 0 {
        return Ok(None);
    }

    Ok(Some(buffer.trim_end().to_string()))
}
