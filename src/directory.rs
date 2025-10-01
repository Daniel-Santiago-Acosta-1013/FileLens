use crate::formatting::format_size;
use crate::ui::{base_table, header_cell};
use comfy_table::{Cell, Color, Row, Table};
use console::style;
use std::cmp::Ordering;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

const DIRECTORY_COUNT_LIMIT: usize = 500;

#[derive(Clone)]
pub struct EntrySummary {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub metadata: Metadata,
}

impl EntrySummary {
    pub fn from_fs_entry(entry: fs::DirEntry) -> Option<Self> {
        let path = entry.path();
        let metadata = entry.metadata().ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        Some(Self {
            name,
            path,
            kind: EntryKind::from(&metadata),
            metadata,
        })
    }
}

#[derive(Clone)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl EntryKind {
    pub fn from(metadata: &Metadata) -> Self {
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            Self::Directory
        } else if file_type.is_file() {
            Self::File
        } else if file_type.is_symlink() {
            Self::Symlink
        } else {
            Self::Other
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Directory => "DIR",
            Self::File => "FILE",
            Self::Symlink => "LINK",
            Self::Other => "OTRO",
        }
    }

    pub fn order_weight(&self) -> usize {
        match self {
            Self::Directory => 0,
            Self::Symlink => 1,
            Self::File => 2,
            Self::Other => 3,
        }
    }
}

pub fn read_directory(path: &Path) -> Result<Vec<EntrySummary>, String> {
    let read_dir = fs::read_dir(path)
        .map_err(|error| format!("No se pudo listar `{}`: {error}", path.display()))?;

    let mut entries: Vec<EntrySummary> = read_dir
        .filter_map(|entry| entry.ok())
        .filter_map(EntrySummary::from_fs_entry)
        .collect();

    entries.sort_by(compare_entries);
    Ok(entries)
}

fn compare_entries(a: &EntrySummary, b: &EntrySummary) -> Ordering {
    match (a.kind.order_weight(), b.kind.order_weight()) {
        (wa, wb) if wa != wb => wa.cmp(&wb),
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    }
}

pub fn render_directory_table(entries: &[EntrySummary], current_dir: &Path) -> Result<(), String> {
    if entries.is_empty() {
        println!(
            "\n{}\n",
            style(format!(
                "[{}] {}",
                current_dir.display(),
                "Directorio vacío"
            ))
            .dim()
        );
        return Ok(());
    }

    let mut table = directories_table();
    for (index, entry) in entries.iter().enumerate() {
        let detail = entry_detail(entry)?;
        table.add_row(Row::from(vec![
            Cell::new(format!("{:>2}", index + 1)).fg(Color::White),
            Cell::new(&entry.name).fg(Color::White),
            Cell::new(entry.kind.badge()).fg(Color::Cyan),
            Cell::new(detail).fg(Color::White),
        ]));
    }

    println!(
        "\n{}",
        style(format!("Contenido de {}", current_dir.display()))
            .cyan()
            .bold()
    );
    println!("{table}\n");
    Ok(())
}

fn directories_table() -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("#"),
        header_cell("Nombre"),
        header_cell("Tipo"),
        header_cell("Detalle"),
    ]);
    table
}

fn entry_detail(entry: &EntrySummary) -> Result<String, String> {
    match entry.kind {
        EntryKind::Directory => {
            let (count, truncated) = count_directory_entries(&entry.path)?;
            if truncated {
                Ok(format!("{count}+ elementos"))
            } else {
                Ok(format!("{count} elementos"))
            }
        }
        EntryKind::File => Ok(format_size(entry.metadata.len())),
        EntryKind::Symlink => {
            let target = fs::read_link(&entry.path)
                .map(|target| target.display().to_string())
                .unwrap_or_else(|_| "Destino no disponible".to_string());
            Ok(format!("→ {target}"))
        }
        EntryKind::Other => Ok("Tipo especial".to_string()),
    }
}

pub fn count_directory_entries(path: &Path) -> Result<(usize, bool), String> {
    let read_dir = fs::read_dir(path).map_err(|error| {
        format!(
            "No se pudo contar los elementos de `{}`: {error}",
            path.display()
        )
    })?;

    let mut count = 0;
    let mut truncated = false;

    for entry in read_dir.take(DIRECTORY_COUNT_LIMIT + 1) {
        if entry.is_ok() {
            count += 1;
        }
    }

    if count > DIRECTORY_COUNT_LIMIT {
        truncated = true;
        count = DIRECTORY_COUNT_LIMIT;
    }

    Ok((count, truncated))
}
