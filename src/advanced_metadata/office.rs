//! Lectura de metadata en documentos Office empaquetados en ZIP.

use comfy_table::Color;
use console::style;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Despliega metadata sensible encontrada en un documento Office y devuelve si hubo hallazgos.
pub fn extract_office_metadata(path: &Path) -> bool {
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

    if let Ok(mut core_file) = archive.by_name("docProps/core.xml") {
        let mut contents = String::new();
        if core_file.read_to_string(&mut contents).is_ok() {
            if let Some(creator) = extract_xml_tag(&contents, "dc:creator") {
                print_office_property("⚠  Creador", &creator, Color::Yellow);
                has_data = true;
            }

            if let Some(last_modified_by) = extract_xml_tag(&contents, "cp:lastModifiedBy") {
                print_office_property(
                    "⚠  Última modificación por",
                    &last_modified_by,
                    Color::Yellow,
                );
                has_data = true;
            }

            if let Some(created) = extract_xml_tag(&contents, "dcterms:created") {
                print_office_property("Fecha de creación", &created, Color::White);
                has_data = true;
            }

            if let Some(modified) = extract_xml_tag(&contents, "dcterms:modified") {
                print_office_property("Fecha de modificación", &modified, Color::White);
                has_data = true;
            }

            if let Some(title) = extract_xml_tag(&contents, "dc:title") {
                print_office_property("Título", &title, Color::White);
                has_data = true;
            }

            if let Some(subject) = extract_xml_tag(&contents, "dc:subject") {
                print_office_property("Asunto", &subject, Color::White);
                has_data = true;
            }

            if let Some(revision) = extract_xml_tag(&contents, "cp:revision") {
                print_office_property("Revisión", &revision, Color::White);
                has_data = true;
            }
        }
    }

    if let Ok(mut app_file) = archive.by_name("docProps/app.xml") {
        let mut contents = String::new();
        if app_file.read_to_string(&mut contents).is_ok() {
            if let Some(app) = extract_xml_tag(&contents, "Application") {
                print_office_property("Aplicación", &app, Color::White);
                has_data = true;
            }

            if let Some(company) = extract_xml_tag(&contents, "Company") {
                print_office_property("⚠  Empresa", &company, Color::Yellow);
                has_data = true;
            }

            if let Some(pages) = extract_xml_tag(&contents, "Pages") {
                print_office_property("Páginas", &pages, Color::White);
                has_data = true;
            }

            if let Some(words) = extract_xml_tag(&contents, "Words") {
                print_office_property("Palabras", &words, Color::White);
                has_data = true;
            }
        }
    }

    has_data
}

fn print_office_property(label: &str, value: &str, color: Color) {
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
