//! Limpieza masiva de metadata para directorios completos con opciones interactivas.

use console::style;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::removal::remove_all_metadata;

/// Filtros disponibles para seleccionar qué archivos se procesarán.
#[derive(Clone, Copy)]
pub enum DirectoryFilter {
    Todos,
    SoloImagenes,
    SoloOffice,
}

impl DirectoryFilter {
    fn matches(self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        let ext = ext.to_lowercase();

        match self {
            DirectoryFilter::Todos => is_supported_image(&ext) || is_supported_office(&ext),
            DirectoryFilter::SoloImagenes => is_supported_image(&ext),
            DirectoryFilter::SoloOffice => is_supported_office(&ext),
        }
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];
const OFFICE_EXTENSIONS: &[&str] = &["docx", "xlsx", "pptx"];
const PDF_EXTENSIONS: &[&str] = &["pdf"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "wmv"];
const CODE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "java", "c", "h", "cpp", "hpp", "cs", "go", "rb", "php",
    "swift", "kt", "kts", "scala", "sh", "bash", "zsh", "html", "css", "scss", "json", "yml",
    "yaml", "toml", "ini", "env", "gradle", "dart",
];
const TEXT_EXTENSIONS: &[&str] = &["txt", "md", "log", "rtf"];
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "tgz"];
const NO_EXTENSION_LABEL: &str = "sin extensión";

#[derive(Default)]
struct DirectoryAnalysis {
    total_files: usize,
    images_count: usize,
    office_count: usize,
    image_extensions: BTreeSet<String>,
    office_extensions: BTreeSet<String>,
    extension_counts: BTreeMap<String, usize>,
}

impl DirectoryAnalysis {
    fn supported_total(&self) -> usize {
        self.images_count + self.office_count
    }

    fn record_extension(&mut self, ext: Option<&str>) {
        let key = ext
            .map(|e| e.to_string())
            .unwrap_or_else(|| NO_EXTENSION_LABEL.to_string());
        *self.extension_counts.entry(key).or_insert(0) += 1;
    }
}

fn is_supported_image(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

fn is_supported_office(ext: &str) -> bool {
    OFFICE_EXTENSIONS.contains(&ext)
}

/// Ejecuta el flujo interactivo para limpiar metadata de un directorio.
pub fn run_directory_cleanup(path: &Path) -> Result<(), String> {
    println!(
        "{}",
        style("\n┌─ Limpieza Masiva de Metadata ─").cyan().bold()
    );
    println!(
        "{}",
        style(format!("│ Directorio seleccionado: {}", path.display())).cyan()
    );
    println!("{}", style("└─").cyan());

    let recursive = prompt_recursive()?;

    let analysis = analyze_directory_content(path, recursive)?;

    if analysis.total_files == 0 {
        println!(
            "\n{}",
            style("│ El directorio no contiene archivos para analizar.").yellow()
        );
        return Ok(());
    }

    render_directory_analysis(&analysis);

    if analysis.supported_total() == 0 {
        println!(
            "\n{}",
            style("│ No se detectaron imágenes ni documentos Office compatibles para limpieza.")
                .yellow()
        );
        return Ok(());
    }

    let Some(filter) = prompt_filter(&analysis)? else {
        println!(
            "\n{}",
            style("│ Operación cancelada por el usuario.").yellow()
        );
        return Ok(());
    };

    let mut files = collect_candidate_files(path, recursive, filter)?;

    if files.is_empty() {
        println!(
            "\n{}",
            style("│ No se encontraron archivos compatibles para limpiar.").yellow()
        );
        return Ok(());
    }

    files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    println!("\n{}", style("┌─ Resumen de limpieza ─").cyan());
    println!(
        "{}",
        style(format!("│ Total de archivos: {}", files.len())).cyan()
    );
    println!(
        "{}",
        style(format!(
            "│ Incluir subdirectorios: {}",
            if recursive { "Sí" } else { "No" }
        ))
        .cyan()
    );
    println!(
        "{}",
        style(format!(
            "│ Filtro aplicado: {}",
            match filter {
                DirectoryFilter::Todos => "Todos los archivos soportados",
                DirectoryFilter::SoloImagenes => "Solo imágenes",
                DirectoryFilter::SoloOffice => "Solo documentos Office",
            }
        ))
        .cyan()
    );

    for path in files.iter().take(3) {
        println!("{}", style(format!("│   • {}", path.display())).dim());
    }
    if files.len() > 3 {
        println!("{}", style("│   • ...").dim());
    }
    println!("{}", style("└─").cyan());

    if !prompt_confirmation()? {
        println!(
            "\n{}",
            style("│ Operación cancelada por el usuario.").yellow()
        );
        return Ok(());
    }

    process_files(files)
}

fn prompt_filter(analysis: &DirectoryAnalysis) -> Result<Option<DirectoryFilter>, String> {
    loop {
        println!("\n{}", style("┌─ ¿Qué archivos deseas limpiar? ─").cyan());
        println!(
            "{}",
            style(format!(
                "│  [1] Todos los archivos soportados ({})",
                analysis.supported_total()
            ))
            .cyan()
        );

        let images_line = if analysis.images_count > 0 {
            style(format!("│  [2] Solo imágenes ({})", analysis.images_count)).cyan()
        } else {
            style("│  [2] Solo imágenes (no se detectaron)".to_string()).dim()
        };
        println!("{}", images_line);

        let office_line = if analysis.office_count > 0 {
            style(format!(
                "│  [3] Solo documentos Office ({})",
                analysis.office_count
            ))
            .cyan()
        } else {
            style("│  [3] Solo documentos Office (no se detectaron)".to_string()).dim()
        };
        println!("{}", office_line);

        println!("{}", style("│  [0] Cancelar").cyan());
        println!("{}", style("└─").cyan());

        print!("\n{}", style("│ Selecciona una opción ▸ ").cyan());
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .map_err(|e| format!("No se pudo leer la opción: {}", e))?;

        match choice.trim() {
            "1" => {
                if analysis.supported_total() == 0 {
                    println!(
                        "\n{}",
                        style("│ No hay archivos compatibles para limpiar.").yellow()
                    );
                    continue;
                }
                return Ok(Some(DirectoryFilter::Todos));
            }
            "2" => {
                if analysis.images_count == 0 {
                    println!(
                        "\n{}",
                        style("│ No se detectaron imágenes soportadas.").yellow()
                    );
                    continue;
                }
                return Ok(Some(DirectoryFilter::SoloImagenes));
            }
            "3" => {
                if analysis.office_count == 0 {
                    println!(
                        "\n{}",
                        style("│ No se detectaron documentos Office soportados.").yellow()
                    );
                    continue;
                }
                return Ok(Some(DirectoryFilter::SoloOffice));
            }
            "0" => return Ok(None),
            _ => println!("\n{}", style("│ Opción inválida.").yellow()),
        }
    }
}

fn prompt_recursive() -> Result<bool, String> {
    loop {
        print!(
            "\n{}",
            style("│ ¿Deseas incluir subdirectorios? (s/n) ▸ ").cyan()
        );
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut response = String::new();
        io::stdin()
            .read_line(&mut response)
            .map_err(|e| format!("No se pudo leer la respuesta: {}", e))?;

        match response.trim().to_lowercase().as_str() {
            "s" | "si" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            "" => return Ok(false),
            _ => println!("\n{}", style("│ Respuesta no reconocida.").yellow()),
        }
    }
}

fn prompt_confirmation() -> Result<bool, String> {
    loop {
        print!(
            "\n{}",
            style("│ ¿Confirmas la limpieza de metadata? (s/n) ▸ ").cyan()
        );
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("No se pudo leer la respuesta: {}", e))?;

        match input.trim().to_lowercase().as_str() {
            "s" | "si" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            "" => return Ok(false),
            _ => println!("\n{}", style("│ Respuesta no reconocida.").yellow()),
        }
    }
}

fn collect_candidate_files(
    root: &Path,
    recursive: bool,
    filter: DirectoryFilter,
) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err("La ruta proporcionada no es un directorio".to_string());
    }

    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut files = Vec::new();

    while let Some(dir) = queue.pop_front() {
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| format!("Entrada inválida en {}: {}", dir.display(), e))?;
            let path = entry.path();

            if path.is_dir() {
                if recursive {
                    queue.push_back(path);
                }
                continue;
            }

            if filter.matches(&path) {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn analyze_directory_content(root: &Path, recursive: bool) -> Result<DirectoryAnalysis, String> {
    if !root.is_dir() {
        return Err("La ruta proporcionada no es un directorio".to_string());
    }

    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut analysis = DirectoryAnalysis::default();

    while let Some(dir) = queue.pop_front() {
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("No se pudo leer {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| format!("Entrada inválida en {}: {}", dir.display(), e))?;
            let path = entry.path();

            if path.is_dir() {
                if recursive {
                    queue.push_back(path);
                }
                continue;
            }

            analysis.total_files += 1;

            let ext_owned = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            let ext = ext_owned.as_deref();

            if let Some(ext) = ext {
                if is_supported_image(ext) {
                    analysis.images_count += 1;
                    analysis.image_extensions.insert(ext.to_string());
                }
                if is_supported_office(ext) {
                    analysis.office_count += 1;
                    analysis.office_extensions.insert(ext.to_string());
                }
            }

            analysis.record_extension(ext);
        }
    }

    Ok(analysis)
}

fn render_directory_analysis(analysis: &DirectoryAnalysis) {
    println!("\n{}", style("┌─ Archivos detectados ─").cyan());
    println!(
        "{}",
        style(format!(
            "│ Total analizado → {} archivos",
            analysis.total_files
        ))
        .cyan()
    );

    let mut items: Vec<_> = analysis.extension_counts.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    for (ext, count) in items {
        let label = format_extension_label(ext);
        let highlight =
            analysis.image_extensions.contains(ext) || analysis.office_extensions.contains(ext);

        let styled_line = if highlight {
            style(format!("│ {} → {} archivos", label, count)).cyan()
        } else {
            style(format!("│ {} → {} archivos", label, count)).white()
        };

        println!("{}", styled_line);
    }

    let category_specs: [(&str, &[&str]); 6] = [
        ("Documentos PDF", PDF_EXTENSIONS),
        ("Audio", AUDIO_EXTENSIONS),
        ("Video", VIDEO_EXTENSIONS),
        ("Código fuente", CODE_EXTENSIONS),
        ("Texto/Markdown", TEXT_EXTENSIONS),
        ("Comprimidos/Paquetes", ARCHIVE_EXTENSIONS),
    ];

    for (label, exts) in category_specs {
        let (count, present) = collect_category_summary(&analysis.extension_counts, exts);
        if count == 0 {
            continue;
        }

        println!(
            "{}",
            style(format!(
                "│ {} → {} archivos [{}]",
                label,
                count,
                present.join(", ")
            ))
            .dim()
        );
    }

    if analysis.supported_total() > 0 {
        let images_info = if analysis.images_count > 0 {
            format!(
                "{} ({})",
                style("imágenes").cyan(),
                analysis
                    .image_extensions
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            style("imágenes").dim().to_string()
        };

        let office_info = if analysis.office_count > 0 {
            format!(
                "{} ({})",
                style("documentos Office").cyan(),
                analysis
                    .office_extensions
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            style("documentos Office").dim().to_string()
        };

        println!(
            "{}",
            style(format!(
                "│ FileLens puede limpiar metadata de: {} y {}",
                images_info, office_info
            ))
            .white()
        );
    } else {
        println!(
            "{}",
            style("│ FileLens puede eliminar metadata de imágenes y documentos Office.").dim()
        );
        println!(
            "{}",
            style("│ No se detectaron archivos de esos tipos en este directorio.").yellow()
        );
    }

    println!("{}", style("└─").cyan());
}

fn format_extension_label(ext: &str) -> String {
    if ext == NO_EXTENSION_LABEL {
        "Sin extensión".to_string()
    } else {
        format!(".{}", ext)
    }
}

fn collect_category_summary(
    counts: &BTreeMap<String, usize>,
    extensions: &[&str],
) -> (usize, Vec<String>) {
    let mut total = 0;
    let mut present = Vec::new();

    for &ext in extensions {
        if let Some(count) = counts.get(ext) {
            total += *count;
            present.push(ext.to_string());
        }
    }

    (total, present)
}

fn process_files(files: Vec<PathBuf>) -> Result<(), String> {
    println!("\n{}", style("┌─ Iniciando limpieza ─").cyan());

    let total = files.len();
    let mut successes = 0_usize;
    let mut failures = 0_usize;

    for (index, path) in files.iter().enumerate() {
        println!(
            "{}",
            style(format!(
                "│ Procesando [{}/{}] {}",
                index + 1,
                total,
                path.display()
            ))
            .cyan()
        );

        match remove_all_metadata(path) {
            Ok(()) => {
                println!("{}", style("│   ✔ Metadata eliminada").green());
                successes += 1;
            }
            Err(error) => {
                println!("{}", style(format!("│   ✖ Error: {}", error)).red());
                failures += 1;
            }
        }
    }

    println!("{}", style("└─").cyan());

    println!("\n{}", style("┌─ Resumen ─").cyan());
    println!("{}", style(format!("│ Exitosos: {}", successes)).cyan());
    println!("{}", style(format!("│ Errores: {}", failures)).cyan());
    println!("{}", style("└─").cyan());

    Ok(())
}
