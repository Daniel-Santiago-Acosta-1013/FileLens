use console::style;
use std::fs;
use std::io::Cursor;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use xmltree::{Element, EmitterConfig, XMLNode};

const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const CP_NS: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const DCTERMS_NS: &str = "http://purl.org/dc/terms/";
const APP_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

const CORE_SANITIZE_FIELDS: [(&str, &str); 11] = [
    ("dc:creator", ""),
    ("cp:lastModifiedBy", ""),
    ("dcterms:created", ""),
    ("dcterms:modified", ""),
    ("dc:title", ""),
    ("dc:subject", ""),
    ("dc:description", ""),
    ("cp:keywords", ""),
    ("cp:category", ""),
    ("cp:contentStatus", ""),
    ("cp:revision", "1"),
];

const APP_SANITIZE_FIELDS: [(&str, &str); 6] = [
    ("Application", ""),
    ("Company", ""),
    ("Manager", ""),
    ("Pages", "0"),
    ("Words", "0"),
    ("Lines", "0"),
];

const CUSTOM_PROPERTIES_EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"/>\n";

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

    // Crear archivo temporal
    let temp_path = generate_temp_filename(path);

    // Guardar sin metadata en archivo temporal
    img.save(&temp_path)
        .map_err(|e| format!("No se pudo guardar la imagen limpia: {}", e))?;

    // Verificar que la metadata fue eliminada
    let metadata_clean = verify_image_metadata_clean(&temp_path)?;

    if !metadata_clean {
        // Limpiar archivo temporal
        let _ = fs::remove_file(&temp_path);

        println!("\n{}", style("┌─ Verificación de metadata fallida ─").red());
        println!(
            "{}",
            style("│ No se pudo confirmar la limpieza del archivo.").red()
        );
        println!(
            "{}",
            style("│ La metadata original podría seguir presente.").red()
        );
        println!("{}", style("└─").red());

        return Err(
            "La verificación indicó que la metadata no se eliminó correctamente".to_string(),
        );
    }

    // Reemplazar el archivo original con el limpio
    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("No se pudo reemplazar el archivo original: {}", e)
    })?;

    println!(
        "\n{}",
        style("┌─ Metadata Eliminada Exitosamente ─").green()
    );
    println!(
        "{}",
        style(format!("│ Archivo: {}", path.display()))
            .green()
            .bold()
    );
    println!(
        "{}",
        style("│ La metadata ha sido eliminada del archivo original.").green()
    );
    println!("{}", style("└─").green());

    Ok(())
}

fn remove_office_metadata(path: &Path) -> Result<(), String> {
    println!(
        "\n{}",
        style("│ Eliminando metadata de documento Office...").dim()
    );

    let temp_path = generate_temp_filename(path);

    let cleaned_anything = rewrite_docx(path, &temp_path, |name, contents| match name {
        "docProps/core.xml" => {
            sanitize_core_properties(contents).map_err(|e| format!("core.xml: {}", e))
        }
        "docProps/app.xml" => {
            sanitize_app_properties(contents).map_err(|e| format!("app.xml: {}", e))
        }
        "docProps/custom.xml" => Ok(sanitize_custom_properties(contents)),
        _ => Ok((contents, false)),
    })?;

    let metadata_clean = verify_office_metadata_clean(&temp_path)?;

    if !metadata_clean {
        let _ = fs::remove_file(&temp_path);

        println!("\n{}", style("┌─ Verificación de metadata fallida ─").red());
        println!(
            "{}",
            style("│ No se pudo confirmar la limpieza del archivo.").red()
        );
        println!(
            "{}",
            style("│ La metadata original podría seguir presente.").red()
        );
        println!("{}", style("└─").red());

        return Err(
            "La verificación indicó que la metadata no se eliminó correctamente".to_string(),
        );
    }

    // Reemplazar el archivo original
    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("No se pudo reemplazar el archivo original: {}", e)
    })?;

    if cleaned_anything {
        println!(
            "\n{}",
            style("┌─ Metadata Eliminada Exitosamente ─").green()
        );
        println!(
            "{}",
            style(format!("│ Archivo: {}", path.display()))
                .green()
                .bold()
        );
        println!(
            "{}",
            style("│ Se eliminaron: Autor, Fechas, Revisiones, Empresa").green()
        );
        println!(
            "{}",
            style("│ La metadata ha sido eliminada del archivo original.").green()
        );
        println!("{}", style("└─").green());
    } else {
        println!(
            "\n{}",
            style("┌─ No se detectó metadata sensible ─").yellow()
        );
        println!(
            "{}",
            style(format!("│ Archivo: {}", path.display()))
                .yellow()
                .bold()
        );
        println!(
            "{}",
            style("│ El contenido permanece sin cambios.").yellow()
        );
        println!("{}", style("└─").yellow());
    }

    Ok(())
}

fn rewrite_docx<F>(path: &Path, output_path: &Path, mut transform: F) -> Result<bool, String>
where
    F: FnMut(&str, Vec<u8>) -> Result<(Vec<u8>, bool), String>,
{
    use std::fs::File;
    use zip::write::FileOptions;
    use zip::{ZipArchive, ZipWriter};

    let source_file =
        File::open(path).map_err(|e| format!("No se pudo abrir el archivo: {}", e))?;
    let mut archive = ZipArchive::new(source_file)
        .map_err(|e| format!("No es un documento Office válido: {}", e))?;

    let target_file =
        File::create(output_path).map_err(|e| format!("No se pudo crear archivo limpio: {}", e))?;
    let mut writer = ZipWriter::new(target_file);

    let mut modified_any = false;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Error leyendo archivo del ZIP: {}", e))?;
        let name = file.name().to_string();

        let mut options = FileOptions::<'_, ()>::default().compression_method(file.compression());
        if let Some(mode) = file.unix_mode() {
            options = options.unix_permissions(mode);
        }
        if let Some(time) = file.last_modified() {
            options = options.last_modified_time(time);
        }

        if file.is_dir() {
            writer
                .add_directory(name, options)
                .map_err(|e| format!("Error creando directorio en ZIP: {}", e))?;
            continue;
        }

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| format!("Error leyendo contenido: {}", e))?;

        let (data_to_write, changed) = transform(&name, contents)?;
        if changed {
            modified_any = true;
        }

        writer
            .start_file(name, options)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
        writer
            .write_all(&data_to_write)
            .map_err(|e| format!("Error escribiendo contenido: {}", e))?;
    }

    writer
        .finish()
        .map_err(|e| format!("Error finalizando archivo: {}", e))?;

    Ok(modified_any)
}

fn sanitize_core_properties(contents: Vec<u8>) -> Result<(Vec<u8>, bool), String> {
    apply_xml_updates(contents, &CORE_SANITIZE_FIELDS, core_field_spec)
}

fn sanitize_app_properties(contents: Vec<u8>) -> Result<(Vec<u8>, bool), String> {
    apply_xml_updates(contents, &APP_SANITIZE_FIELDS, app_field_spec)
}

fn sanitize_custom_properties(contents: Vec<u8>) -> (Vec<u8>, bool) {
    let sanitized = CUSTOM_PROPERTIES_EMPTY.as_bytes().to_vec();
    let modified = contents != sanitized;
    (sanitized, modified)
}

fn verify_image_metadata_clean(path: &Path) -> Result<bool, String> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(path)
        .map_err(|e| format!("No se pudo abrir la imagen limpia para verificación: {}", e))?;
    let mut reader = BufReader::new(file);

    match exif::Reader::new().read_from_container(&mut reader) {
        Ok(exif) => Ok(exif.fields().next().is_none()),
        Err(exif::Error::NotFound(_)) | Err(exif::Error::BlankValue(_)) => Ok(true),
        Err(exif::Error::InvalidFormat(_)) => Ok(true),
        Err(exif::Error::Io(err)) => Err(format!(
            "No se pudo leer metadata EXIF durante la verificación: {}",
            err
        )),
        Err(other) => Err(format!("Error verificando metadata EXIF: {}", other)),
    }
}

fn verify_office_metadata_clean(path: &Path) -> Result<bool, String> {
    use std::fs::File;
    use zip::ZipArchive;
    use zip::result::ZipError;

    let file = File::open(path)
        .map_err(|e| format!("No se pudo abrir archivo limpio para verificación: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("No es un documento Office válido: {}", e))?;

    let core_clean = match archive.by_name("docProps/core.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("No se pudo leer core.xml durante la verificación: {}", e))?;
            is_xml_metadata_clean(&contents, &CORE_SANITIZE_FIELDS, core_field_spec)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a core.xml durante la verificación: {}",
                e
            ));
        }
    };

    let app_clean = match archive.by_name("docProps/app.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("No se pudo leer app.xml durante la verificación: {}", e))?;
            is_xml_metadata_clean(&contents, &APP_SANITIZE_FIELDS, app_field_spec)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a app.xml durante la verificación: {}",
                e
            ));
        }
    };

    let custom_clean = match archive.by_name("docProps/custom.xml") {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).map_err(|e| {
                format!("No se pudo leer custom.xml durante la verificación: {}", e)
            })?;
            is_custom_metadata_clean(&contents)?
        }
        Err(ZipError::FileNotFound) => true,
        Err(e) => {
            return Err(format!(
                "No se pudo acceder a custom.xml durante la verificación: {}",
                e
            ));
        }
    };

    Ok(core_clean && app_clean && custom_clean)
}

fn is_xml_metadata_clean(
    contents: &[u8],
    expected_values: &[(&str, &str)],
    lookup: fn(&str) -> Option<FieldSpec<'static>>,
) -> Result<bool, String> {
    let root = Element::parse(Cursor::new(contents)).map_err(|e| {
        format!(
            "Error leyendo XML de metadata durante la verificación: {}",
            e
        )
    })?;

    for &(tag, expected) in expected_values {
        if let Some(spec) = lookup(tag)
            && !element_matches_expected_value(&root, spec, expected)
        {
            return Ok(false);
        }
    }

    Ok(true)
}

fn is_custom_metadata_clean(contents: &[u8]) -> Result<bool, String> {
    if contents == CUSTOM_PROPERTIES_EMPTY.as_bytes() {
        return Ok(true);
    }

    let root = Element::parse(Cursor::new(contents))
        .map_err(|e| format!("Error leyendo custom.xml durante la verificación: {}", e))?;

    let has_property_elements = root
        .children
        .iter()
        .any(|node| matches!(node, XMLNode::Element(_)));
    let has_text = root.children.iter().any(|node| {
        if let XMLNode::Text(text) = node {
            !text.trim().is_empty()
        } else {
            false
        }
    });

    Ok(!has_property_elements && !has_text)
}

fn element_matches_expected_value(root: &Element, spec: FieldSpec<'_>, expected: &str) -> bool {
    for node in root.children.iter() {
        if let XMLNode::Element(child) = node
            && element_matches(child, &spec)
        {
            let content = element_text_content(child);
            return if expected.is_empty() {
                content.is_empty()
            } else {
                content == expected
            };
        }
    }

    expected.is_empty()
}

fn element_text_content(element: &Element) -> String {
    let mut content = String::new();
    for node in element.children.iter() {
        if let XMLNode::Text(text) = node {
            content.push_str(text);
        }
    }
    content.trim().to_string()
}

fn apply_xml_updates(
    contents: Vec<u8>,
    updates: &[(&str, &str)],
    lookup: fn(&str) -> Option<FieldSpec<'static>>,
) -> Result<(Vec<u8>, bool), String> {
    let mut root = Element::parse(Cursor::new(&contents[..]))
        .map_err(|e| format!("Error leyendo XML de metadata: {}", e))?;

    let mut modified = false;
    for &(tag, value) in updates {
        if let Some(spec) = lookup(tag) {
            modified |= apply_update_to_element(&mut root, spec, value);
        }
    }

    if !modified {
        return Ok((contents, false));
    }

    let mut output = Vec::new();
    let mut config = EmitterConfig::new();
    config.perform_indent = false;
    config.write_document_declaration = true;
    root.write_with_config(&mut output, config)
        .map_err(|e| format!("Error escribiendo XML sanitizado: {}", e))?;

    Ok((output, true))
}

#[derive(Clone, Copy)]
struct FieldSpec<'a> {
    prefix: Option<&'a str>,
    local_name: &'a str,
    namespace: Option<&'a str>,
}

fn core_field_spec(tag: &str) -> Option<FieldSpec<'static>> {
    match tag {
        "dc:creator" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "creator",
            namespace: Some(DC_NS),
        }),
        "cp:lastModifiedBy" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "lastModifiedBy",
            namespace: Some(CP_NS),
        }),
        "dcterms:created" => Some(FieldSpec {
            prefix: Some("dcterms"),
            local_name: "created",
            namespace: Some(DCTERMS_NS),
        }),
        "dcterms:modified" => Some(FieldSpec {
            prefix: Some("dcterms"),
            local_name: "modified",
            namespace: Some(DCTERMS_NS),
        }),
        "dc:title" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "title",
            namespace: Some(DC_NS),
        }),
        "dc:subject" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "subject",
            namespace: Some(DC_NS),
        }),
        "dc:description" => Some(FieldSpec {
            prefix: Some("dc"),
            local_name: "description",
            namespace: Some(DC_NS),
        }),
        "cp:keywords" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "keywords",
            namespace: Some(CP_NS),
        }),
        "cp:category" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "category",
            namespace: Some(CP_NS),
        }),
        "cp:contentStatus" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "contentStatus",
            namespace: Some(CP_NS),
        }),
        "cp:revision" => Some(FieldSpec {
            prefix: Some("cp"),
            local_name: "revision",
            namespace: Some(CP_NS),
        }),
        _ => None,
    }
}

fn app_field_spec(tag: &str) -> Option<FieldSpec<'static>> {
    match tag {
        "Application" => Some(FieldSpec {
            prefix: None,
            local_name: "Application",
            namespace: Some(APP_NS),
        }),
        "Company" => Some(FieldSpec {
            prefix: None,
            local_name: "Company",
            namespace: Some(APP_NS),
        }),
        "Manager" => Some(FieldSpec {
            prefix: None,
            local_name: "Manager",
            namespace: Some(APP_NS),
        }),
        "Pages" => Some(FieldSpec {
            prefix: None,
            local_name: "Pages",
            namespace: Some(APP_NS),
        }),
        "Words" => Some(FieldSpec {
            prefix: None,
            local_name: "Words",
            namespace: Some(APP_NS),
        }),
        "Lines" => Some(FieldSpec {
            prefix: None,
            local_name: "Lines",
            namespace: Some(APP_NS),
        }),
        _ => None,
    }
}

fn apply_update_to_element(root: &mut Element, spec: FieldSpec<'_>, new_value: &str) -> bool {
    for node in root.children.iter_mut() {
        if let XMLNode::Element(child) = node
            && element_matches(child, &spec)
        {
            return set_element_text(child, new_value);
        }
    }

    // Crear elemento si no existía y se proporciona algún valor
    let mut new_child = Element::new(spec.local_name);
    if let Some(prefix) = spec.prefix {
        new_child.prefix = Some(prefix.to_string());
    }
    if let Some(namespace) = spec.namespace {
        new_child.namespace = Some(namespace.to_string());
    }
    if !new_value.is_empty() {
        new_child
            .children
            .push(XMLNode::Text(new_value.to_string()));
    }
    root.children.push(XMLNode::Element(new_child));
    true
}

fn element_matches(element: &Element, spec: &FieldSpec<'_>) -> bool {
    if element.name != spec.local_name {
        return false;
    }

    match (spec.namespace, element.namespace.as_deref()) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn set_element_text(element: &mut Element, new_value: &str) -> bool {
    let current = element
        .children
        .iter()
        .find_map(|node| match node {
            XMLNode::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or("");
    if current == new_value {
        return false;
    }

    element
        .children
        .retain(|node| !matches!(node, XMLNode::Text(_)));

    if !new_value.is_empty() {
        element.children.push(XMLNode::Text(new_value.to_string()));
    }

    true
}

fn apply_office_metadata_edit(path: &Path, xml_tag: &str, value: &str) -> Result<(), String> {
    enum DocPropsTarget {
        Core,
        App,
    }

    let target = if xml_tag.contains(':') {
        DocPropsTarget::Core
    } else {
        DocPropsTarget::App
    };

    let temp_path = generate_temp_filename(path);

    let changed = rewrite_docx(path, &temp_path, |name, contents| match (name, &target) {
        ("docProps/core.xml", DocPropsTarget::Core) => {
            let updates = [(xml_tag, value); 1];
            apply_xml_updates(contents, &updates, core_field_spec)
        }
        ("docProps/app.xml", DocPropsTarget::App) => {
            let updates = [(xml_tag, value); 1];
            apply_xml_updates(contents, &updates, app_field_spec)
        }
        _ => Ok((contents, false)),
    })?;

    if !changed {
        let _ = fs::remove_file(&temp_path);
        return Err("No se encontró el campo solicitado para modificar".to_string());
    }

    // Reemplazar el archivo original
    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        format!("No se pudo reemplazar el archivo original: {}", e)
    })?;

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

fn generate_temp_filename(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();

    // Usar timestamp para evitar colisiones
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    parent.join(format!(".{}_temp_{}.{}", stem, timestamp, extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Write};
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    #[test]
    fn remove_office_metadata_clears_docprops() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let source = dir.path().join("sample.docx");
        create_sample_docx(&source)?;

        remove_office_metadata(&source)?;

        // Ahora verifica el archivo original que fue modificado in-place
        assert!(source.exists());
        assert!(
            verify_office_metadata_clean(&source)
                .expect("la verificación del documento limpio falló")
        );

        let mut archive = ZipArchive::new(File::open(&source)?)?;

        let mut core_contents = String::new();
        archive
            .by_name("docProps/core.xml")?
            .read_to_string(&mut core_contents)?;
        assert!(!core_contents.contains("Autor Prueba"));
        assert!(!core_contents.contains("Editor Prueba"));
        assert!(core_contents.contains("<cp:revision>1</cp:revision>"));

        let mut app_contents = String::new();
        archive
            .by_name("docProps/app.xml")?
            .read_to_string(&mut app_contents)?;
        assert!(!app_contents.contains("Microsoft Word"));
        assert!(app_contents.contains("<Pages>0</Pages>"));

        let mut custom_contents = String::new();
        archive
            .by_name("docProps/custom.xml")?
            .read_to_string(&mut custom_contents)?;
        assert!(custom_contents.trim().ends_with("docPropsVTypes\"/>"));

        Ok(())
    }

    #[test]
    fn remove_image_metadata_strips_exif() -> Result<(), Box<dyn std::error::Error>> {
        const SAMPLE_IMAGE_WITH_EXIF: &[u8] = include_bytes!("../tests/data/exif_sample.png");

        let dir = tempdir()?;
        let source = dir.path().join("sample.png");

        std::fs::write(&source, SAMPLE_IMAGE_WITH_EXIF)?;

        remove_image_metadata(&source)?;

        // Ahora verifica el archivo original que fue modificado in-place
        assert!(source.exists());
        assert!(
            verify_image_metadata_clean(&source)
                .expect("la verificacion de la imagen limpia fallo"),
            "la imagen generada deberia quedar sin metadata detectable"
        );

        Ok(())
    }

    #[test]
    fn verify_office_metadata_clean_flags_dirty_doc() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let source = dir.path().join("sample.docx");
        create_sample_docx(&source)?;

        let is_clean = verify_office_metadata_clean(&source)
            .expect("la verificación del documento original no debería fallar");
        assert!(!is_clean);

        Ok(())
    }

    #[test]
    fn apply_office_metadata_edit_updates_author() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let source = dir.path().join("sample.docx");
        create_sample_docx(&source)?;

        apply_office_metadata_edit(&source, "dc:creator", "Nuevo Autor")
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

        assert!(source.exists());

        let mut archive = ZipArchive::new(File::open(&source)?)?;
        let mut core_contents = String::new();
        archive
            .by_name("docProps/core.xml")?
            .read_to_string(&mut core_contents)?;

        assert!(core_contents.contains("<dc:creator>Nuevo Autor</dc:creator>"));

        Ok(())
    }

    fn create_sample_docx(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
    <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
    <Default Extension="xml" ContentType="application/xml"/>
    <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
    <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
    <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
    <Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/>
</Types>
"#;

        const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
    <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#;

        const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p><w:r><w:t>Documento de prueba</w:t></w:r></w:p>
    </w:body>
</w:document>
"#;

        const CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/"
                   xmlns:dcmitype="http://purl.org/dc/dcmitype/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <dc:creator>Autor Prueba</dc:creator>
    <cp:lastModifiedBy>Editor Prueba</cp:lastModifiedBy>
    <dcterms:created xsi:type="dcterms:W3CDTF">2024-01-01T00:00:00Z</dcterms:created>
    <dcterms:modified xsi:type="dcterms:W3CDTF">2024-02-01T00:00:00Z</dcterms:modified>
    <dc:title>Documento Demo</dc:title>
    <dc:subject>Asunto Demo</dc:subject>
    <cp:revision>6</cp:revision>
</cp:coreProperties>
"#;

        const APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <Application>Microsoft Word</Application>
    <Company>Compania Demo</Company>
    <Pages>2</Pages>
    <Words>345</Words>
    <Lines>12</Lines>
</Properties>
"#;

        const CUSTOM_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="CustomField">
        <vt:lpwstr>Dato Confidencial</vt:lpwstr>
    </property>
</Properties>
"#;

        let file = File::create(path)?;
        let mut writer = ZipWriter::new(file);
        let options =
            FileOptions::<'_, ()>::default().compression_method(CompressionMethod::Stored);

        writer.start_file("[Content_Types].xml", options)?;
        writer.write_all(CONTENT_TYPES.as_bytes())?;

        writer.start_file("_rels/.rels", options)?;
        writer.write_all(RELS_XML.as_bytes())?;

        writer.start_file("word/document.xml", options)?;
        writer.write_all(DOCUMENT_XML.as_bytes())?;

        writer.start_file("docProps/core.xml", options)?;
        writer.write_all(CORE_XML.as_bytes())?;

        writer.start_file("docProps/app.xml", options)?;
        writer.write_all(APP_XML.as_bytes())?;

        writer.start_file("docProps/custom.xml", options)?;
        writer.write_all(CUSTOM_XML.as_bytes())?;

        writer.finish()?;

        Ok(())
    }
}
