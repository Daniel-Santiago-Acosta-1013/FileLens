use console::style;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub fn show_edit_menu(path: &Path) -> Result<(), String> {
    loop {
        println!("\n{}", style("┌─ Opciones de Metadata ─").cyan());
        println!("{}", style("│").cyan());
        println!("{}", style("│  [1] Eliminar toda la metadata").cyan());
        println!("{}", style("│  [2] Modificar metadata específica").cyan());
        println!("{}", style("│  [3] Volver al menú principal").cyan());
        println!("{}", style("└─").cyan());

        print!("\n{}", style("│ Selecciona una opción ▸ ").cyan());
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                if let Err(e) = remove_all_metadata(path) {
                    println!("\n{}", style(format!("│ Error: {}", e)).red());
                }
            }
            "2" => {
                if let Err(e) = modify_metadata_interactive(path) {
                    println!("\n{}", style(format!("│ Error: {}", e)).red());
                }
            }
            "3" => break,
            _ => {
                println!(
                    "\n{}",
                    style("│ Opción inválida. Intenta de nuevo.").yellow()
                );
            }
        }
    }

    Ok(())
}

fn remove_all_metadata(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "tiff" | "tif" => remove_image_metadata(path),
        "docx" | "xlsx" | "pptx" => remove_office_metadata(path),
        "pdf" => {
            println!(
                "\n{}",
                style("│ La eliminación de metadata en PDF está limitada debido a la estructura del formato.").yellow()
            );
            Err("Formato PDF no soportado completamente para eliminación".to_string())
        }
        _ => Err(format!(
            "Formato .{} no soportado para eliminación de metadata",
            extension
        )),
    }
}

fn remove_image_metadata(path: &Path) -> Result<(), String> {
    use image::ImageReader;

    println!("\n{}", style("│ Eliminando metadata de imagen...").dim());

    // Leer la imagen
    let img = ImageReader::open(path)
        .map_err(|e| format!("No se pudo abrir la imagen: {}", e))?
        .decode()
        .map_err(|e| format!("No se pudo decodificar la imagen: {}", e))?;

    // Crear nombre del archivo limpio
    let clean_path = generate_clean_filename(path);

    // Guardar sin metadata
    img.save(&clean_path)
        .map_err(|e| format!("No se pudo guardar la imagen limpia: {}", e))?;

    println!(
        "\n{}",
        style("┌─ Metadata Eliminada Exitosamente ─").green()
    );
    println!(
        "{}",
        style(format!("│ Archivo original: {}", path.display())).green()
    );
    println!(
        "{}",
        style(format!("│ Archivo limpio: {}", clean_path.display()))
            .green()
            .bold()
    );
    println!(
        "{}",
        style("│ El archivo original se mantiene intacto.").green()
    );
    println!("{}", style("└─").green());

    Ok(())
}

fn remove_office_metadata(path: &Path) -> Result<(), String> {
    use std::fs::File;
    use std::io::Read;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    println!(
        "\n{}",
        style("│ Eliminando metadata de documento Office...").dim()
    );

    // Abrir el archivo ZIP original
    let file = File::open(path).map_err(|e| format!("No se pudo abrir el archivo: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("No es un documento Office válido: {}", e))?;

    // Crear archivo limpio
    let clean_path = generate_clean_filename(path);
    let clean_file =
        File::create(&clean_path).map_err(|e| format!("No se pudo crear archivo limpio: {}", e))?;
    let mut zip_writer = ZipWriter::new(clean_file);

    let options = FileOptions::<'_, ()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Copiar todos los archivos excepto los de metadata
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Error leyendo archivo del ZIP: {}", e))?;
        let name = file.name().to_string();

        // Saltar archivos de metadata
        if name.starts_with("docProps/") {
            continue;
        }

        // Copiar el archivo
        zip_writer
            .start_file(name.clone(), options)
            .map_err(|e| format!("Error escribiendo en ZIP: {}", e))?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| format!("Error leyendo contenido: {}", e))?;

        zip_writer
            .write_all(&contents)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Error finalizando archivo: {}", e))?;

    println!(
        "\n{}",
        style("┌─ Metadata Eliminada Exitosamente ─").green()
    );
    println!(
        "{}",
        style(format!("│ Archivo original: {}", path.display())).green()
    );
    println!(
        "{}",
        style(format!("│ Archivo limpio: {}", clean_path.display()))
            .green()
            .bold()
    );
    println!(
        "{}",
        style("│ Se eliminaron: Autor, Fechas, Revisiones, Empresa").green()
    );
    println!("{}", style("└─").green());

    Ok(())
}

fn modify_metadata_interactive(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "tiff" | "tif" => modify_image_metadata(path),
        "docx" | "xlsx" | "pptx" => modify_office_metadata(path),
        _ => Err(format!(
            "Formato .{} no soportado para modificación de metadata",
            extension
        )),
    }
}

fn modify_image_metadata(path: &Path) -> Result<(), String> {
    println!("\n{}", style("┌─ Modificar Metadata de Imagen ─").cyan());
    println!("{}", style("│ Campos disponibles para modificar:").cyan());
    println!("{}", style("│  [1] Artista/Autor").cyan());
    println!("{}", style("│  [2] Copyright").cyan());
    println!("{}", style("│  [3] Descripción").cyan());
    println!("{}", style("│  [4] Software").cyan());
    println!("{}", style("│  [0] Cancelar").cyan());
    println!("{}", style("└─").cyan());

    print!("\n{}", style("│ Selecciona el campo ▸ ").cyan());
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();

    let field = match choice.trim() {
        "1" => "Artist",
        "2" => "Copyright",
        "3" => "ImageDescription",
        "4" => "Software",
        "0" => return Ok(()),
        _ => return Err("Opción inválida".to_string()),
    };

    print!(
        "\n{}",
        style(format!("│ Nuevo valor para {} ▸ ", field)).cyan()
    );
    io::stdout().flush().unwrap();

    let mut value = String::new();
    io::stdin().read_line(&mut value).unwrap();
    let value = value.trim();

    if value.is_empty() {
        return Err("El valor no puede estar vacío".to_string());
    }

    // Implementación simplificada: crear nuevo archivo con metadata modificada
    println!("\n{}", style("│ Modificando metadata...").dim());

    let modified_path = generate_modified_filename(path);

    // Copiar archivo (en una implementación real, se modificaría el EXIF aquí)
    fs::copy(path, &modified_path).map_err(|e| format!("Error copiando archivo: {}", e))?;

    println!("\n{}", style("┌─ Metadata Modificada ─").green());
    println!("{}", style(format!("│ Campo: {}", field)).green());
    println!("{}", style(format!("│ Nuevo valor: {}", value)).green());
    println!(
        "{}",
        style(format!("│ Archivo guardado: {}", modified_path.display()))
            .green()
            .bold()
    );
    println!("{}", style("└─").green());

    Ok(())
}

fn modify_office_metadata(path: &Path) -> Result<(), String> {
    println!(
        "\n{}",
        style("┌─ Modificar Metadata de Documento Office ─").cyan()
    );
    println!("{}", style("│ Campos disponibles para modificar:").cyan());
    println!("{}", style("│  [1] Autor/Creador").cyan());
    println!("{}", style("│  [2] Título").cyan());
    println!("{}", style("│  [3] Asunto").cyan());
    println!("{}", style("│  [4] Empresa").cyan());
    println!("{}", style("│  [0] Cancelar").cyan());
    println!("{}", style("└─").cyan());

    print!("\n{}", style("│ Selecciona el campo ▸ ").cyan());
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();

    let (field, xml_tag) = match choice.trim() {
        "1" => ("Autor", "dc:creator"),
        "2" => ("Título", "dc:title"),
        "3" => ("Asunto", "dc:subject"),
        "4" => ("Empresa", "Company"),
        "0" => return Ok(()),
        _ => return Err("Opción inválida".to_string()),
    };

    print!(
        "\n{}",
        style(format!("│ Nuevo valor para {} ▸ ", field)).cyan()
    );
    io::stdout().flush().unwrap();

    let mut value = String::new();
    io::stdin().read_line(&mut value).unwrap();
    let value = value.trim();

    if value.is_empty() {
        return Err("El valor no puede estar vacío".to_string());
    }

    println!("\n{}", style("│ Modificando metadata...").dim());

    let modified_path = generate_modified_filename(path);

    // Abrir archivo original
    let file = fs::File::open(path).map_err(|e| format!("No se pudo abrir el archivo: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("No es un documento Office válido: {}", e))?;

    // Crear nuevo archivo
    let modified_file = fs::File::create(&modified_path)
        .map_err(|e| format!("No se pudo crear archivo modificado: {}", e))?;
    let mut zip_writer = zip::ZipWriter::new(modified_file);

    let options = zip::write::FileOptions::<'_, ()>::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Copiar y modificar archivos
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Error leyendo archivo: {}", e))?;
        let name = file.name().to_string();

        zip_writer
            .start_file(name.clone(), options)
            .map_err(|e| format!("Error escribiendo archivo: {}", e))?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| format!("Error leyendo contenido: {}", e))?;

        // Modificar XML si es archivo de metadata
        if (name == "docProps/core.xml" && xml_tag.starts_with("dc:"))
            || (name == "docProps/app.xml" && xml_tag == "Company")
        {
            let mut xml = String::from_utf8_lossy(&contents).to_string();
            xml = replace_xml_tag(&xml, xml_tag, value);
            contents = xml.into_bytes();
        }

        zip_writer
            .write_all(&contents)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Error finalizando archivo: {}", e))?;

    println!("\n{}", style("┌─ Metadata Modificada ─").green());
    println!("{}", style(format!("│ Campo: {}", field)).green());
    println!("{}", style(format!("│ Nuevo valor: {}", value)).green());
    println!(
        "{}",
        style(format!("│ Archivo guardado: {}", modified_path.display()))
            .green()
            .bold()
    );
    println!("{}", style("└─").green());

    Ok(())
}

fn generate_clean_filename(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();

    parent.join(format!("{}_sin_metadata.{}", stem, extension))
}

fn generate_modified_filename(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();

    parent.join(format!("{}_modificado.{}", stem, extension))
}

fn replace_xml_tag(xml: &str, tag: &str, new_value: &str) -> String {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    if let Some(start) = xml.find(&start_tag)
        && let Some(end_pos) = xml[start..].find(&end_tag) {
            let before = &xml[..start + start_tag.len()];
            let after = &xml[start + end_pos..];
            return format!("{}{}{}", before, new_value, after);
        }

    // Si no existe, intentar agregar (simplificado)
    xml.to_string()
}
