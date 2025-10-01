use crate::ui::base_table;
use comfy_table::{Attribute, Cell, Color, Row, Table};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn extract_image_metadata(path: &Path) -> Option<Table> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut bufreader).ok()?;

    let mut table = base_table();
    table.set_header(vec![
        Cell::new("Campo EXIF")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Valor")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Descripción")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
    ]);

    let mut has_data = false;

    // Información de la cámara
    if let Some(field) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Fabricante",
            field.display_value().to_string(),
            "Marca del dispositivo",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Modelo",
            field.display_value().to_string(),
            "Modelo del dispositivo",
        ));
        has_data = true;
    }

    // Información del software
    if let Some(field) = exif.get_field(exif::Tag::Software, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Software",
            field.display_value().to_string(),
            "Software usado para editar",
        ));
        has_data = true;
    }

    // Fecha y hora
    if let Some(field) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Fecha/Hora",
            field.display_value().to_string(),
            "Cuando se capturó la imagen",
        ));
        has_data = true;
    }

    // Configuración de la cámara
    if let Some(field) = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Apertura",
            field.display_value().to_string(),
            "Apertura del lente (f-stop)",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Exposición",
            field.display_value().to_string(),
            "Tiempo de exposición",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
        table.add_row(property_row(
            "ISO",
            field.display_value().to_string(),
            "Sensibilidad ISO",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Distancia focal",
            field.display_value().to_string(),
            "Longitud focal del lente",
        ));
        has_data = true;
    }

    // GPS - CRÍTICO PARA PRIVACIDAD
    if let Some(lat) = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
        && let Some(lat_ref) = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)
    {
        table.add_row(property_row_warning(
            "GPS Latitud",
            format!("{} {}", lat.display_value(), lat_ref.display_value()),
            "⚠ UBICACIÓN SENSIBLE",
        ));
        has_data = true;
    }

    if let Some(lon) = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
        && let Some(lon_ref) = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)
    {
        table.add_row(property_row_warning(
            "GPS Longitud",
            format!("{} {}", lon.display_value(), lon_ref.display_value()),
            "⚠ UBICACIÓN SENSIBLE",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY) {
        table.add_row(property_row_warning(
            "GPS Altitud",
            field.display_value().to_string(),
            "⚠ UBICACIÓN SENSIBLE",
        ));
        has_data = true;
    }

    // Orientación
    if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Orientación",
            field.display_value().to_string(),
            "Rotación de la imagen",
        ));
        has_data = true;
    }

    // Dimensiones
    if let Some(field) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Ancho",
            field.display_value().to_string(),
            "Ancho en píxeles",
        ));
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Alto",
            field.display_value().to_string(),
            "Alto en píxeles",
        ));
        has_data = true;
    }

    // Artista/Autor
    if let Some(field) = exif.get_field(exif::Tag::Artist, exif::In::PRIMARY) {
        table.add_row(property_row_warning(
            "Artista",
            field.display_value().to_string(),
            "⚠ INFORMACIÓN PERSONAL",
        ));
        has_data = true;
    }

    // Copyright
    if let Some(field) = exif.get_field(exif::Tag::Copyright, exif::In::PRIMARY) {
        table.add_row(property_row(
            "Copyright",
            field.display_value().to_string(),
            "Derechos de autor",
        ));
        has_data = true;
    }

    if has_data { Some(table) } else { None }
}

pub fn extract_pdf_metadata(_path: &Path) -> Option<Table> {
    // Implementación simplificada - requiere biblioteca más compatible
    None
}

pub fn extract_office_metadata(path: &Path) -> Option<Table> {
    // Detectar si es un documento Office (ZIP-based)
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut table = base_table();
    table.set_header(vec![
        Cell::new("Propiedad Office")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Valor")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Nota")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
    ]);

    let mut has_data = false;

    // Buscar core.xml (metadata principal)
    if let Ok(mut core_file) = archive.by_name("docProps/core.xml") {
        use std::io::Read;
        let mut contents = String::new();
        core_file.read_to_string(&mut contents).ok()?;

        // Extraer información básica (parsing simple XML)
        if let Some(creator) = extract_xml_tag(&contents, "dc:creator") {
            table.add_row(property_row_warning(
                "Creador",
                creator,
                "⚠ INFORMACIÓN PERSONAL",
            ));
            has_data = true;
        }

        if let Some(last_modified_by) = extract_xml_tag(&contents, "cp:lastModifiedBy") {
            table.add_row(property_row_warning(
                "Última modificación por",
                last_modified_by,
                "⚠ INFORMACIÓN PERSONAL",
            ));
            has_data = true;
        }

        if let Some(created) = extract_xml_tag(&contents, "dcterms:created") {
            table.add_row(property_row(
                "Fecha de creación",
                created,
                "Cuándo se creó el documento",
            ));
            has_data = true;
        }

        if let Some(modified) = extract_xml_tag(&contents, "dcterms:modified") {
            table.add_row(property_row(
                "Fecha de modificación",
                modified,
                "Última modificación",
            ));
            has_data = true;
        }

        if let Some(title) = extract_xml_tag(&contents, "dc:title") {
            table.add_row(property_row("Título", title, "Título del documento"));
            has_data = true;
        }

        if let Some(subject) = extract_xml_tag(&contents, "dc:subject") {
            table.add_row(property_row("Asunto", subject, "Tema del documento"));
            has_data = true;
        }

        if let Some(revision) = extract_xml_tag(&contents, "cp:revision") {
            table.add_row(property_row("Revisión", revision, "Número de revisión"));
            has_data = true;
        }
    }

    // Buscar app.xml (metadata de aplicación)
    if let Ok(mut app_file) = archive.by_name("docProps/app.xml") {
        use std::io::Read;
        let mut contents = String::new();
        app_file.read_to_string(&mut contents).ok()?;

        if let Some(app) = extract_xml_tag(&contents, "Application") {
            table.add_row(property_row("Aplicación", app, "Software usado para crear"));
            has_data = true;
        }

        if let Some(company) = extract_xml_tag(&contents, "Company") {
            table.add_row(property_row_warning(
                "Empresa",
                company,
                "⚠ INFORMACIÓN ORGANIZACIONAL",
            ));
            has_data = true;
        }

        if let Some(pages) = extract_xml_tag(&contents, "Pages") {
            table.add_row(property_row("Páginas", pages, "Número de páginas"));
            has_data = true;
        }

        if let Some(words) = extract_xml_tag(&contents, "Words") {
            table.add_row(property_row("Palabras", words, "Conteo de palabras"));
            has_data = true;
        }
    }

    if has_data { Some(table) } else { None }
}

fn property_row(label: &str, value: String, description: &str) -> Row {
    Row::from(vec![
        Cell::new(label).fg(Color::Cyan),
        Cell::new(value).fg(Color::White),
        Cell::new(description).fg(Color::DarkGrey),
    ])
}

fn property_row_warning(label: &str, value: String, description: &str) -> Row {
    Row::from(vec![
        Cell::new(label).fg(Color::Yellow),
        Cell::new(value).fg(Color::White),
        Cell::new(description).fg(Color::Red),
    ])
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)? + start;

    Some(xml[start..end].trim().to_string())
}
