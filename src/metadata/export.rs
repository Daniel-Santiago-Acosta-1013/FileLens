//! Exportacion de reportes de metadata en distintos formatos.

use crate::metadata::report::{EntryLevel, MetadataReport, ReportEntry};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub enum ExportFormat {
    Json,
    Txt,
    Xlsx,
    Pdf,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Txt => "txt",
            ExportFormat::Xlsx => "xlsx",
            ExportFormat::Pdf => "pdf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Json => "JSON",
            ExportFormat::Txt => "TXT",
            ExportFormat::Xlsx => "Excel",
            ExportFormat::Pdf => "PDF",
        }
    }
}

pub fn parse_export_format(input: &str) -> Result<ExportFormat, String> {
    match input.to_lowercase().as_str() {
        "json" => Ok(ExportFormat::Json),
        "txt" | "text" => Ok(ExportFormat::Txt),
        "xlsx" | "excel" => Ok(ExportFormat::Xlsx),
        "pdf" => Ok(ExportFormat::Pdf),
        _ => Err("Formato de exportacion no reconocido".to_string()),
    }
}

pub fn export_metadata_report(
    report: &MetadataReport,
    format: ExportFormat,
    path: &Path,
) -> Result<(), String> {
    match format {
        ExportFormat::Json => export_json(report, path),
        ExportFormat::Txt => export_txt(report, path),
        ExportFormat::Xlsx => export_xlsx(report, path),
        ExportFormat::Pdf => export_pdf(report, path),
    }
}

fn export_json(report: &MetadataReport, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|err| format!("No se pudo serializar JSON: {err}"))?;
    fs::write(path, json).map_err(|err| format!("No se pudo guardar el JSON: {err}"))
}

fn export_txt(report: &MetadataReport, path: &Path) -> Result<(), String> {
    let mut output = String::new();
    output.push_str("Reporte de metadata\n");
    output.push_str("===================\n\n");

    append_txt_section(&mut output, "Sistema", &report.system, None);

    for section in &report.internal {
        append_txt_section(
            &mut output,
            &section.title,
            &section.entries,
            section.notice.as_ref().map(|n| n.message.as_str()),
        );
    }

    if !report.risks.is_empty() {
        append_txt_section(&mut output, "Riesgos", &report.risks, None);
    }

    if !report.errors.is_empty() {
        output.push_str("Errores\n");
        output.push_str("-------\n");
        for error in &report.errors {
            output.push_str(&format!("- {error}\n"));
        }
        output.push('\n');
    }

    fs::write(path, output).map_err(|err| format!("No se pudo guardar el TXT: {err}"))
}

fn append_txt_section(
    output: &mut String,
    title: &str,
    entries: &[ReportEntry],
    notice: Option<&str>,
) {
    output.push_str(title);
    output.push('\n');
    output.push_str(&"-".repeat(title.len()));
    output.push('\n');

    if entries.is_empty() {
        output.push_str("(Sin datos)\n\n");
        return;
    }

    for entry in entries {
        let level = level_label(entry.level);
        output.push_str(&format!("- {}: {} ({})\n", entry.label, entry.value, level));
    }

    if let Some(note) = notice {
        output.push_str(&format!("Nota: {note}\n"));
    }
    output.push('\n');
}

fn export_xlsx(report: &MetadataReport, path: &Path) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Metadata")
        .map_err(|err| format!("No se pudo crear hoja de calculo: {err}"))?;

    worksheet
        .set_column_width(0, 20.0)
        .map_err(|err| format!("No se pudo ajustar columnas: {err}"))?;
    worksheet
        .set_column_width(1, 32.0)
        .map_err(|err| format!("No se pudo ajustar columnas: {err}"))?;
    worksheet
        .set_column_width(2, 70.0)
        .map_err(|err| format!("No se pudo ajustar columnas: {err}"))?;
    worksheet
        .set_column_width(3, 14.0)
        .map_err(|err| format!("No se pudo ajustar columnas: {err}"))?;

    let header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1F4E78))
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);

    let cell_format = Format::new()
        .set_text_wrap()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left);

    let level_format = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    worksheet
        .write_with_format(0, 0, "Seccion", &header_format)
        .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
    worksheet
        .write_with_format(0, 1, "Etiqueta", &header_format)
        .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
    worksheet
        .write_with_format(0, 2, "Valor", &header_format)
        .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
    worksheet
        .write_with_format(0, 3, "Nivel", &header_format)
        .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;

    let rows = collect_rows(report);
    for (index, row) in rows.iter().enumerate() {
        let row_index = (index + 1) as u32;
        worksheet
            .write_with_format(row_index, 0, row.section.as_str(), &cell_format)
            .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
        worksheet
            .write_with_format(row_index, 1, row.label.as_str(), &cell_format)
            .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
        worksheet
            .write_with_format(row_index, 2, row.value.as_str(), &cell_format)
            .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
        worksheet
            .write_with_format(row_index, 3, row.level.as_str(), &level_format)
            .map_err(|err| format!("No se pudo escribir el XLSX: {err}"))?;
    }

    workbook
        .save(path)
        .map_err(|err| format!("No se pudo guardar el XLSX: {err}"))
}

fn export_pdf(report: &MetadataReport, path: &Path) -> Result<(), String> {
    let lines = build_pdf_lines(report);

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_regular_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let font_bold_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_regular_id,
            "F2" => font_bold_id,
        },
    });

    let page_width = 595_i64;
    let page_height = 842_i64;
    let margin_left = 50_i64;
    let margin_top = 60_i64;
    let margin_bottom = 60_i64;

    let mut page_ids = Vec::new();
    let mut page_ops: Vec<Operation> = Vec::new();
    let mut current_y = page_height - margin_top;

    for line in lines {
        let line_height = line.size + 4;
        if current_y - line_height < margin_bottom {
            let content_id = add_pdf_page_content(&mut doc, &page_ops);
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
            });
            page_ids.push(page_id);
            page_ops.clear();
            current_y = page_height - margin_top;
        }

        if line.text.trim().is_empty() {
            current_y -= line_height;
            continue;
        }

        let x = margin_left + line.indent;
        let font_name = match line.font {
            PdfFont::Regular => "F1",
            PdfFont::Bold => "F2",
        };

        page_ops.push(Operation::new("BT", vec![]));
        page_ops.push(Operation::new("Tf", vec![font_name.into(), line.size.into()]));
        page_ops.push(Operation::new("Td", vec![x.into(), current_y.into()]));
        page_ops.push(Operation::new(
            "Tj",
            vec![Object::string_literal(line.text.as_str())],
        ));
        page_ops.push(Operation::new("ET", vec![]));

        current_y -= line_height;
    }

    let content_id = add_pdf_page_content(&mut doc, &page_ops);
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
    });
    page_ids.push(page_id);

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids.iter().map(|id| (*id).into()).collect::<Vec<Object>>(),
        "Count" => page_ids.len() as i64,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), page_width.into(), page_height.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.compress();

    doc.save(path)
        .map(|_| ())
        .map_err(|err| format!("No se pudo guardar el PDF: {err}"))
}

fn add_pdf_page_content(doc: &mut Document, ops: &[Operation]) -> lopdf::ObjectId {
    let content = Content {
        operations: ops.to_vec(),
    };
    doc.add_object(Stream::new(
        dictionary! {},
        content.encode().unwrap_or_default(),
    ))
}

struct ExportRow {
    section: String,
    label: String,
    value: String,
    level: String,
}

fn collect_rows(report: &MetadataReport) -> Vec<ExportRow> {
    let mut rows = Vec::new();
    rows.extend(section_rows("Sistema", &report.system, None));
    for section in &report.internal {
        rows.extend(section_rows(
            &section.title,
            &section.entries,
            section.notice.as_ref().map(|n| n.message.as_str()),
        ));
    }
    if !report.risks.is_empty() {
        rows.extend(section_rows("Riesgos", &report.risks, None));
    }
    if !report.errors.is_empty() {
        for error in &report.errors {
            rows.push(ExportRow {
                section: "Errores".to_string(),
                label: "Error".to_string(),
                value: error.to_string(),
                level: "Error".to_string(),
            });
        }
    }
    rows
}

fn section_rows(
    title: &str,
    entries: &[ReportEntry],
    notice: Option<&str>,
) -> Vec<ExportRow> {
    let mut rows = Vec::new();
    if entries.is_empty() {
        rows.push(ExportRow {
            section: title.to_string(),
            label: "Sin datos".to_string(),
            value: "-".to_string(),
            level: "Info".to_string(),
        });
        return rows;
    }
    for entry in entries {
        rows.push(ExportRow {
            section: title.to_string(),
            label: entry.label.clone(),
            value: entry.value.clone(),
            level: level_label(entry.level).to_string(),
        });
    }
    if let Some(note) = notice {
        rows.push(ExportRow {
            section: title.to_string(),
            label: "Nota".to_string(),
            value: note.to_string(),
            level: "Info".to_string(),
        });
    }
    rows
}

#[derive(Clone, Copy)]
enum PdfFont {
    Regular,
    Bold,
}

struct PdfLine {
    text: String,
    font: PdfFont,
    size: i64,
    indent: i64,
}

fn build_pdf_lines(report: &MetadataReport) -> Vec<PdfLine> {
    let mut lines = Vec::new();
    lines.push(PdfLine {
        text: "Reporte de metadata".to_string(),
        font: PdfFont::Bold,
        size: 18,
        indent: 0,
    });
    lines.push(PdfLine {
        text: " ".to_string(),
        font: PdfFont::Regular,
        size: 6,
        indent: 0,
    });

    lines.extend(section_pdf_lines("Sistema", &report.system, None));

    for section in &report.internal {
        lines.extend(section_pdf_lines(
            &section.title,
            &section.entries,
            section.notice.as_ref().map(|n| n.message.as_str()),
        ));
    }

    if !report.risks.is_empty() {
        lines.extend(section_pdf_lines("Riesgos", &report.risks, None));
    }

    if !report.errors.is_empty() {
        lines.push(PdfLine {
            text: "Errores".to_string(),
            font: PdfFont::Bold,
            size: 13,
            indent: 0,
        });
        for error in &report.errors {
            let entry = format!("- {error}");
            lines.extend(wrap_pdf_text(entry, PdfFont::Regular, 11, 12, 90));
        }
    }

    lines
}

fn section_pdf_lines(
    title: &str,
    entries: &[ReportEntry],
    notice: Option<&str>,
) -> Vec<PdfLine> {
    let mut lines = Vec::new();
    lines.push(PdfLine {
        text: title.to_string(),
        font: PdfFont::Bold,
        size: 13,
        indent: 0,
    });

    if entries.is_empty() {
        lines.push(PdfLine {
            text: "Sin datos".to_string(),
            font: PdfFont::Regular,
            size: 11,
            indent: 12,
        });
    } else {
        for entry in entries {
            let line = format!("- {}: {}", entry.label, entry.value);
            lines.extend(wrap_pdf_text(line, PdfFont::Regular, 11, 12, 90));
        }
    }

    if let Some(note) = notice {
        let note_line = format!("Nota: {note}");
        lines.extend(wrap_pdf_text(note_line, PdfFont::Regular, 10, 12, 90));
    }

    lines.push(PdfLine {
        text: " ".to_string(),
        font: PdfFont::Regular,
        size: 6,
        indent: 0,
    });

    lines
}

fn wrap_pdf_text(
    text: String,
    font: PdfFont,
    size: i64,
    indent: i64,
    max_chars: usize,
) -> Vec<PdfLine> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        if current.len() + 1 + word.len() > max_chars {
            lines.push(PdfLine {
                text: current,
                font,
                size,
                indent,
            });
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(PdfLine {
            text: current,
            font,
            size,
            indent,
        });
    }
    if lines.is_empty() {
        lines.push(PdfLine {
            text: text.to_string(),
            font,
            size,
            indent,
        });
    }
    lines
}

fn level_label(level: EntryLevel) -> &'static str {
    match level {
        EntryLevel::Info => "Info",
        EntryLevel::Warning => "Advertencia",
        EntryLevel::Success => "Exito",
        EntryLevel::Error => "Error",
        EntryLevel::Muted => "Silenciado",
    }
}
