use crate::directory::{self, EntryKind, EntrySummary};
use crate::metadata;
use crate::ui;
use comfy_table::{Attribute, Cell, Color, Row};
use console::{Key, Term, style};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn run() -> Result<(), String> {
    let mut state =
        AppState::new().map_err(|error| format!("No se pudo inicializar FileLens: {error}"))?;

    let stats = state.refresh_listing()?;
    state.apply_refresh_stats(stats);
    state.set_status(StatusMessage::info(
        "Navega con ↑/↓, abre carpetas o metadata con Enter y regresa con ←. Pulsa q para salir.",
    ));

    let term = Term::stdout();
    term.hide_cursor()
        .map_err(|error| format!("No se pudo preparar la terminal: {error}"))?;

    let result = run_event_loop(&term, &mut state);

    term.show_cursor().ok();
    term.clear_screen().ok();
    term.flush().ok();

    result
}

fn run_event_loop(term: &Term, state: &mut AppState) -> Result<(), String> {
    loop {
        render(term, state)?;

        let key = term
            .read_key()
            .map_err(|error| format!("No se pudo leer la tecla: {error}"))?;

        match key {
            Key::Char('q') | Key::Escape | Key::CtrlC => break,
            Key::ArrowUp => state.move_selection_up(),
            Key::ArrowDown => state.move_selection_down(),
            Key::ArrowLeft | Key::Backspace => match state.go_to_parent() {
                Ok(stats) => {
                    state.apply_refresh_stats(stats);
                    state.set_status(StatusMessage::info(format!(
                        "Ubicación: {}",
                        state.current_dir.display()
                    )));
                }
                Err(message) => state.set_status(StatusMessage::warning(message)),
            },
            Key::ArrowRight | Key::Enter => {
                if let Err(message) = state.activate_selected(term) {
                    state.set_status(StatusMessage::error(message));
                }
            }
            Key::Char('r') | Key::Char('R') => match state.refresh_listing() {
                Ok(stats) => {
                    state.apply_refresh_stats(stats);
                    state.set_status(StatusMessage::info("Lista actualizada"));
                }
                Err(message) => state.set_status(StatusMessage::error(message)),
            },
            Key::Home => state.select_first(),
            Key::End => state.select_last(),
            Key::PageUp => state.jump_up(),
            Key::PageDown => state.jump_down(),
            _ => {}
        }
    }

    Ok(())
}

fn render(term: &Term, state: &AppState) -> Result<(), String> {
    term.clear_screen()
        .map_err(|error| format!("No se pudo limpiar la pantalla: {error}"))?;

    ui::render_header();
    println!(
        "{} {}",
        style("Ubicación").bold().cyan(),
        style(state.current_dir.display()).dim()
    );
    println!(
        "{}",
        style("↑/↓ mover · ← retroceder · →/Enter abrir o ver metadata · r refrescar · q salir")
            .dim()
    );
    println!();

    if state.entries.is_empty() {
        println!("{}", style("Este directorio está vacío.").dim());
    } else {
        let table = build_directory_table(state);
        println!("{table}");

        if let Some(entry) = state.selected_entry() {
            println!();
            render_selected_info(entry);
        }
    }

    let mut printed_status = false;
    if let Some(status) = &state.status {
        println!();
        print_status_line(status);
        printed_status = true;
    }
    if let Some(warning) = &state.refresh_warning {
        if !printed_status {
            println!();
        }
        print_status_line(warning);
    }

    term.flush()
        .map_err(|error| format!("No se pudo actualizar la terminal: {error}"))
}

fn build_directory_table(state: &AppState) -> String {
    let mut table = ui::base_table();
    table.set_header(vec![
        ui::header_cell("#"),
        ui::header_cell("Nombre"),
        ui::header_cell("Tipo"),
        ui::header_cell("Detalle"),
    ]);

    for (index, entry) in state.entries.iter().enumerate() {
        table.add_row(build_row(index, entry, index == state.selected));
    }

    table.to_string()
}

fn build_row(index: usize, entry: &ListedEntry, selected: bool) -> Row {
    let mut index_cell = Cell::new(format!("{:>2}", index + 1)).fg(Color::White);
    let mut name_cell = Cell::new(&entry.summary.name).fg(Color::White);
    let mut type_cell = Cell::new(entry.summary.kind.badge()).fg(Color::Cyan);
    let mut detail_cell = Cell::new(&entry.detail).fg(Color::White);

    if selected {
        index_cell = highlight_cell(index_cell);
        name_cell = highlight_cell(name_cell);
        type_cell = highlight_cell(type_cell);
        detail_cell = highlight_cell(detail_cell);
    }

    Row::from(vec![index_cell, name_cell, type_cell, detail_cell])
}

fn highlight_cell(cell: Cell) -> Cell {
    cell.bg(Color::Rgb {
        r: 96,
        g: 160,
        b: 255,
    })
    .fg(Color::Black)
    .add_attribute(Attribute::Bold)
}

fn render_selected_info(entry: &ListedEntry) {
    println!(
        "{} {}",
        style("Seleccionado:").cyan().bold(),
        entry.summary.name
    );
    println!(
        "{} {}",
        style("Tipo:").dim(),
        style(entry.summary.kind.badge()).cyan()
    );
    println!("{} {}", style("Detalle:").dim(), entry.detail);
}

fn print_status_line(message: &StatusMessage) {
    match message.kind {
        StatusKind::Info => println!("{}", style(&message.text).green()),
        StatusKind::Warning => println!("{}", style(&message.text).yellow()),
        StatusKind::Error => println!("{}", style(&message.text).red().bold()),
    }
}

fn show_metadata(term: &Term, path: &Path) -> Result<(), String> {
    term.clear_screen()
        .map_err(|error| format!("No se pudo limpiar la pantalla: {error}"))?;
    ui::render_header();
    metadata::render_metadata(path)?;
    println!();
    println!(
        "{}",
        style("Pulsa cualquier tecla para volver a la navegación.").dim()
    );
    term.flush()
        .map_err(|error| format!("No se pudo actualizar la terminal: {error}"))?;
    term.read_key()
        .map_err(|error| format!("No se pudo leer la tecla: {error}"))?;
    Ok(())
}

fn path_points_to_directory(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

#[derive(Clone)]
struct ListedEntry {
    summary: EntrySummary,
    detail: String,
}

struct RefreshStats {
    warning_count: usize,
}

#[derive(Clone)]
struct StatusMessage {
    text: String,
    kind: StatusKind,
}

#[derive(Clone)]
enum StatusKind {
    Info,
    Warning,
    Error,
}

impl StatusMessage {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Info,
        }
    }

    fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Warning,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Error,
        }
    }
}

struct AppState {
    current_dir: PathBuf,
    entries: Vec<ListedEntry>,
    selected: usize,
    status: Option<StatusMessage>,
    refresh_warning: Option<StatusMessage>,
}

impl AppState {
    fn new() -> io::Result<Self> {
        Ok(Self {
            current_dir: env::current_dir()?,
            entries: Vec::new(),
            selected: 0,
            status: None,
            refresh_warning: None,
        })
    }

    fn refresh_listing(&mut self) -> Result<RefreshStats, String> {
        let summaries = directory::read_directory(&self.current_dir)?;
        let mut entries = Vec::with_capacity(summaries.len());
        let mut warnings = 0usize;

        for summary in summaries {
            let detail = match directory::entry_detail(&summary) {
                Ok(detail) => detail,
                Err(_) => {
                    warnings += 1;
                    "Detalle no disponible".to_string()
                }
            };

            entries.push(ListedEntry { summary, detail });
        }

        self.entries = entries;

        if self.entries.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }

        Ok(RefreshStats {
            warning_count: warnings,
        })
    }

    fn apply_refresh_stats(&mut self, stats: RefreshStats) {
        if stats.warning_count == 0 {
            self.refresh_warning = None;
            return;
        }

        let plural = if stats.warning_count == 1 { "" } else { "s" };
        self.refresh_warning = Some(StatusMessage::warning(format!(
            "Se omitieron detalles para {} elemento{} por falta de permisos.",
            stats.warning_count, plural
        )));
    }

    fn selected_entry(&self) -> Option<&ListedEntry> {
        self.entries.get(self.selected)
    }

    fn set_status(&mut self, status: StatusMessage) {
        self.status = Some(status);
    }

    fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_selection_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    fn jump_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let step = self.selected.min(5);
        self.selected -= step;
    }

    fn jump_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let max_index = self.entries.len() - 1;
        self.selected = self.selected.saturating_add(5).min(max_index);
    }

    fn select_first(&mut self) {
        if !self.entries.is_empty() {
            self.selected = 0;
        }
    }

    fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn go_to_parent(&mut self) -> Result<RefreshStats, String> {
        let parent = self
            .current_dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "Ya estás en el nivel más alto disponible.".to_string())?;

        self.current_dir = parent;
        self.selected = 0;
        self.refresh_listing()
    }

    fn change_directory_to(&mut self, target: PathBuf) -> Result<RefreshStats, String> {
        let metadata = fs::symlink_metadata(&target)
            .map_err(|error| format!("No se pudo acceder a `{}`: {error}", target.display()))?;

        let destination = if metadata.is_dir() {
            target
        } else if metadata.is_symlink() {
            let resolved = fs::canonicalize(&target).map_err(|error| {
                format!(
                    "No se pudo resolver el enlace `{}`: {error}",
                    target.display()
                )
            })?;
            let resolved_metadata = fs::metadata(&resolved).map_err(|error| {
                format!(
                    "No se pudo acceder al destino `{}`: {error}",
                    resolved.display()
                )
            })?;
            if resolved_metadata.is_dir() {
                resolved
            } else {
                return Err("El enlace no apunta a un directorio.".to_string());
            }
        } else {
            return Err(
                "Solo puedes navegar hacia directorios. Usa Enter para ver la metadata."
                    .to_string(),
            );
        };

        self.current_dir = destination;
        self.selected = 0;
        self.refresh_listing()
    }

    fn activate_selected(&mut self, term: &Term) -> Result<(), String> {
        let entry = match self.selected_entry() {
            Some(entry) => entry.clone(),
            None => return Ok(()),
        };

        let should_enter = matches!(entry.summary.kind, EntryKind::Directory)
            || (matches!(entry.summary.kind, EntryKind::Symlink)
                && path_points_to_directory(&entry.summary.path));

        if should_enter {
            let stats = self.change_directory_to(entry.summary.path.clone())?;
            self.apply_refresh_stats(stats);
            self.set_status(StatusMessage::info(format!(
                "Entraste a {}",
                entry.summary.name
            )));
            return Ok(());
        }

        show_metadata(term, &entry.summary.path)?;
        self.set_status(StatusMessage::info(format!(
            "Metadata mostrada para {}",
            entry.summary.name
        )));
        Ok(())
    }
}
