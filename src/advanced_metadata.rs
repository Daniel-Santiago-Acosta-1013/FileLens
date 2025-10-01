use comfy_table::Color;
use console::style;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn extract_image_metadata(path: &Path) -> bool {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut bufreader = BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let exif = match exif_reader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => return false,
    };

    let mut has_data = false;
    println!();

    // Información de la cámara
    if let Some(field) = exif.get_field(exif::Tag::Make, exif::In::PRIMARY) {
        print_exif_property(
            "Fabricante",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::Model, exif::In::PRIMARY) {
        print_exif_property("Modelo", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    // Información del software
    if let Some(field) = exif.get_field(exif::Tag::Software, exif::In::PRIMARY) {
        print_exif_property("Software", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    // Fecha y hora
    if let Some(field) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        print_exif_property(
            "Fecha/Hora",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    // Configuración de la cámara
    if let Some(field) = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
        print_exif_property("Apertura", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
        print_exif_property(
            "Exposición",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
        print_exif_property("ISO", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
        print_exif_property(
            "Distancia focal",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    // GPS - CRÍTICO PARA PRIVACIDAD
    if let Some(lat) = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
        && let Some(lat_ref) = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)
    {
        print_exif_property(
            "⚠  GPS Latitud",
            &format!("{} {}", lat.display_value(), lat_ref.display_value()),
            Color::Yellow,
        );
        has_data = true;
    }

    if let Some(lon) = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
        && let Some(lon_ref) = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)
    {
        print_exif_property(
            "⚠  GPS Longitud",
            &format!("{} {}", lon.display_value(), lon_ref.display_value()),
            Color::Yellow,
        );
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY) {
        print_exif_property(
            "⚠  GPS Altitud",
            &field.display_value().to_string(),
            Color::Yellow,
        );
        has_data = true;
    }

    // Orientación
    if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        print_exif_property(
            "Orientación",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    // Dimensiones
    if let Some(field) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        print_exif_property("Ancho", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        print_exif_property("Alto", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    // Artista/Autor
    if let Some(field) = exif.get_field(exif::Tag::Artist, exif::In::PRIMARY) {
        print_exif_property(
            "⚠  Artista",
            &field.display_value().to_string(),
            Color::Yellow,
        );
        has_data = true;
    }

    // Copyright
    if let Some(field) = exif.get_field(exif::Tag::Copyright, exif::In::PRIMARY) {
        print_exif_property(
            "Copyright",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    has_data
}

pub fn extract_pdf_metadata(_path: &Path) -> bool {
    // Implementación simplificada - requiere biblioteca más compatible
    false
}

pub fn extract_office_metadata(path: &Path) -> bool {
    // Detectar si es un documento Office (ZIP-based)
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return false,
    };

    let mut has_data = false;
    println!();

    // Buscar core.xml (metadata principal)
    if let Ok(mut core_file) = archive.by_name("docProps/core.xml") {
        use std::io::Read;
        let mut contents = String::new();
        if core_file.read_to_string(&mut contents).is_ok() {
            // Extraer información básica (parsing simple XML)
            if let Some(creator) = extract_xml_tag(&contents, "dc:creator") {
                print_exif_property("⚠  Creador", &creator, Color::Yellow);
                has_data = true;
            }

            if let Some(last_modified_by) = extract_xml_tag(&contents, "cp:lastModifiedBy") {
                print_exif_property(
                    "⚠  Última modificación por",
                    &last_modified_by,
                    Color::Yellow,
                );
                has_data = true;
            }

            if let Some(created) = extract_xml_tag(&contents, "dcterms:created") {
                print_exif_property("Fecha de creación", &created, Color::White);
                has_data = true;
            }

            if let Some(modified) = extract_xml_tag(&contents, "dcterms:modified") {
                print_exif_property("Fecha de modificación", &modified, Color::White);
                has_data = true;
            }

            if let Some(title) = extract_xml_tag(&contents, "dc:title") {
                print_exif_property("Título", &title, Color::White);
                has_data = true;
            }

            if let Some(subject) = extract_xml_tag(&contents, "dc:subject") {
                print_exif_property("Asunto", &subject, Color::White);
                has_data = true;
            }

            if let Some(revision) = extract_xml_tag(&contents, "cp:revision") {
                print_exif_property("Revisión", &revision, Color::White);
                has_data = true;
            }
        }
    }

    // Buscar app.xml (metadata de aplicación)
    if let Ok(mut app_file) = archive.by_name("docProps/app.xml") {
        use std::io::Read;
        let mut contents = String::new();
        if app_file.read_to_string(&mut contents).is_ok() {
            if let Some(app) = extract_xml_tag(&contents, "Application") {
                print_exif_property("Aplicación", &app, Color::White);
                has_data = true;
            }

            if let Some(company) = extract_xml_tag(&contents, "Company") {
                print_exif_property("⚠  Empresa", &company, Color::Yellow);
                has_data = true;
            }

            if let Some(pages) = extract_xml_tag(&contents, "Pages") {
                print_exif_property("Páginas", &pages, Color::White);
                has_data = true;
            }

            if let Some(words) = extract_xml_tag(&contents, "Words") {
                print_exif_property("Palabras", &words, Color::White);
                has_data = true;
            }
        }
    }

    has_data
}

fn print_exif_property(label: &str, value: &str, color: Color) {
    let label_styled = style(format!("    {}", label)).cyan().bold();
    let arrow = style("→").dim();
    let value_styled = match color {
        Color::Yellow => style(value).yellow(),
        Color::Green => style(value).green(),
        Color::Red => style(value).red(),
        _ => style(value).white(),
    };
    println!("{} {} {}", label_styled, arrow, value_styled);
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)? + start;

    Some(xml[start..end].trim().to_string())
}
