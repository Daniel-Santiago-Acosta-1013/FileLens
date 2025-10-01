//! Flujo interactivo para modificar metadata específica.

use console::style;
use std::io::{self, Write};
use std::path::Path;

use super::office::apply_office_metadata_edit;

/// Permite editar metadata puntual dependiendo del tipo de archivo.
pub fn modify_metadata_interactive(path: &Path) -> Result<(), String> {
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

fn modify_image_metadata(_path: &Path) -> Result<(), String> {
    println!("\n{}", style("┌─ Modificar Metadata de Imagen ─").cyan());
    println!("{}", style("│").cyan());
    println!(
        "{}",
        style("│ NOTA: La modificación de metadata EXIF en imágenes").yellow()
    );
    println!(
        "{}",
        style("│ requiere bibliotecas especializadas adicionales.").yellow()
    );
    println!("{}", style("│").cyan());
    println!(
        "{}",
        style("│ Por ahora, solo se soporta eliminación de metadata.").dim()
    );
    println!(
        "{}",
        style("│ Use la opción [1] del menú principal para eliminar.").dim()
    );
    println!("{}", style("└─").cyan());

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

    apply_office_metadata_edit(path, xml_tag, value)
        .map_err(|e| format!("No se pudo actualizar la metadata: {}", e))?;

    println!("\n{}", style("┌─ Metadata Modificada ─").green());
    println!("{}", style(format!("│ Campo: {}", field)).green());
    println!("{}", style(format!("│ Nuevo valor: {}", value)).green());
    println!(
        "{}",
        style(format!("│ Archivo: {}", path.display()))
            .green()
            .bold()
    );
    println!(
        "{}",
        style("│ La metadata ha sido modificada en el archivo original.").green()
    );
    println!("{}", style("└─").green());

    Ok(())
}
