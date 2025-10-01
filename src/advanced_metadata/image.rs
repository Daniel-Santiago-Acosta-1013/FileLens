//! Extracción de metadata EXIF relevante para imágenes.

use comfy_table::Color;
use console::style;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Imprime la metadata EXIF detectada y devuelve si se encontró información.
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

    if let Some(field) = exif.get_field(exif::Tag::Software, exif::In::PRIMARY) {
        print_exif_property("Software", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
        print_exif_property(
            "Fecha/Hora",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

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

    if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        print_exif_property(
            "Orientación",
            &field.display_value().to_string(),
            Color::White,
        );
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
        print_exif_property("Ancho", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
        print_exif_property("Alto", &field.display_value().to_string(), Color::White);
        has_data = true;
    }

    if let Some(field) = exif.get_field(exif::Tag::Artist, exif::In::PRIMARY) {
        print_exif_property(
            "⚠  Artista",
            &field.display_value().to_string(),
            Color::Yellow,
        );
        has_data = true;
    }

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
