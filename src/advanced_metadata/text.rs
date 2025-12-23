//! Extracción de metadata para texto plano y CSV.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub fn extract_text_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata TXT");
    let risks = Vec::new();

    let Some(analysis) = analyze_text(path) else {
        section.notice = Some(SectionNotice::new(
            "No se pudo analizar el texto",
            EntryLevel::Warning,
        ));
        return AdvancedMetadataResult { section, risks };
    };

    section
        .entries
        .push(ReportEntry::info("Encoding", analysis.encoding));
    section.entries.push(ReportEntry::info(
        "BOM",
        analysis.bom.unwrap_or_else(|| "No".to_string()),
    ));
    section.entries.push(ReportEntry::info(
        "Saltos de línea",
        analysis.line_endings,
    ));
    section.entries.push(ReportEntry::info(
        "Número de líneas",
        analysis.lines.to_string(),
    ));
    section.entries.push(ReportEntry::info(
        "Longitud promedio de línea",
        format!("{:.2} bytes", analysis.avg_line_len),
    ));
    section.entries.push(ReportEntry::info(
        "Caracteres nulos",
        if analysis.has_nulls { "Sí" } else { "No" },
    ));

    AdvancedMetadataResult { section, risks }
}

pub fn extract_csv_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata CSV");
    let mut risks = Vec::new();

    let Some(analysis) = analyze_text(path) else {
        section.notice = Some(SectionNotice::new(
            "No se pudo analizar el CSV",
            EntryLevel::Warning,
        ));
        return AdvancedMetadataResult { section, risks };
    };

    section
        .entries
        .push(ReportEntry::info("Encoding", analysis.encoding));
    section.entries.push(ReportEntry::info(
        "BOM",
        analysis.bom.unwrap_or_else(|| "No".to_string()),
    ));

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => String::from_utf8_lossy(&analysis.sample).to_string(),
    };

    let sample_lines: Vec<&str> = text.lines().take(20).collect();
    let delimiter = detect_delimiter(&sample_lines);
    let quote = detect_quote(&text);

    section.entries.push(ReportEntry::info(
        "Delimitador",
        format!("{}", delimiter as char),
    ));
    section.entries.push(ReportEntry::info(
        "Quote",
        quote.map(|q| q.to_string()).unwrap_or_else(|| "none".to_string()),
    ));

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut records = reader.records();
    let first = records.next().and_then(|r| r.ok());
    let second = records.next().and_then(|r| r.ok());
    let has_header = match (&first, &second) {
        (Some(a), Some(b)) => guess_header(a, b),
        _ => false,
    };

    let mut header = Vec::new();
    let mut column_stats = Vec::new();
    let mut rows = 0;
    let mut inconsistent = 0;

    if let Some(first) = first {
        if has_header {
            header = first.iter().map(|s| s.to_string()).collect();
        } else {
            ensure_stats(&mut column_stats, first.len());
            process_record(&first, &mut column_stats);
            rows += 1;
        }
    }
    if let Some(second) = second {
        ensure_stats(&mut column_stats, second.len());
        process_record(&second, &mut column_stats);
        rows += 1;
    }
    for record in records.flatten() {
        ensure_stats(&mut column_stats, record.len());
        if record.len() != column_stats.len() {
            inconsistent += 1;
        }
        process_record(&record, &mut column_stats);
        rows += 1;
    }

    let columns = if !header.is_empty() {
        header.len()
    } else {
        column_stats.len()
    };

    section.entries.push(ReportEntry::info(
        "Tiene header",
        if has_header { "Sí" } else { "No" },
    ));
    if !header.is_empty() {
        section.entries.push(ReportEntry::info(
            "Columnas",
            header.join(", "),
        ));
    }
    section
        .entries
        .push(ReportEntry::info("Filas", rows.to_string()));
    section.entries.push(ReportEntry::info(
        "Columnas (conteo)",
        columns.to_string(),
    ));
    if inconsistent > 0 {
        section.entries.push(ReportEntry::warning(
            "Filas inconsistentes",
            inconsistent.to_string(),
        ));
        risks.push(ReportEntry::warning(
            "Filas inconsistentes",
            inconsistent.to_string(),
        ));
    }

    let mut type_entries = Vec::new();
    let mut null_entries = Vec::new();
    for (index, stat) in column_stats.iter().enumerate() {
        let name = header.get(index).cloned().unwrap_or_else(|| format!("Col {index}"));
        type_entries.push(format!("{name}:{:?}", stat.kind));
        if stat.nulls > 0 {
            null_entries.push(format!("{name}:{:?}", stat.nulls));
        }
    }
    if !type_entries.is_empty() {
        section.entries.push(ReportEntry::info(
            "Tipos por columna",
            type_entries.join(", "),
        ));
    }
    if !null_entries.is_empty() {
        section.entries.push(ReportEntry::info(
            "Nulos por columna",
            null_entries.join(", "),
        ));
    }

    AdvancedMetadataResult { section, risks }
}

struct TextAnalysis {
    encoding: String,
    bom: Option<String>,
    line_endings: String,
    lines: usize,
    avg_line_len: f64,
    has_nulls: bool,
    sample: Vec<u8>,
}

fn analyze_text(path: &Path) -> Option<TextAnalysis> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut sample = Vec::new();
    let mut lines = 0;
    let mut total_len = 0usize;
    let mut has_nulls = false;
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let mut prev = 0u8;
    let mut bom_len = 0usize;
    let mut buffer = [0_u8; 8192];
    let mut offset = 0usize;

    loop {
        let bytes = reader.read(&mut buffer).ok()?;
        if bytes == 0 {
            break;
        }
        if sample.len() < 64 * 1024 {
            let remaining = 64 * 1024 - sample.len();
            sample.extend_from_slice(&buffer[..bytes.min(remaining)]);
            if sample.len() >= 2 && bom_len == 0 {
                bom_len = detect_bom(&sample).1;
            }
        }

        for &byte in &buffer[..bytes] {
            if offset < bom_len {
                offset += 1;
                prev = byte;
                continue;
            }
            total_len += 1;
            if byte == 0 {
                has_nulls = true;
            }
            if byte == b'\n' {
                lines += 1;
                if prev == b'\r' {
                    crlf += 1;
                } else {
                    lf += 1;
                }
            } else if byte == b'\r' {
                lines += 1;
                cr += 1;
            }
            prev = byte;
        }
    }

    let (bom, _) = detect_bom(&sample);
    let encoding = if let Some(bom) = &bom {
        bom.clone()
    } else if std::str::from_utf8(&sample).is_ok() {
        "UTF-8".to_string()
    } else {
        "ISO-8859-1 (heurístico)".to_string()
    };
    let avg_line_len = if lines > 0 {
        total_len as f64 / lines as f64
    } else {
        0.0
    };
    let line_endings = format!("LF:{lf}, CRLF:{crlf}, CR:{cr}");

    Some(TextAnalysis {
        encoding,
        bom,
        line_endings,
        lines,
        avg_line_len,
        has_nulls,
        sample,
    })
}

fn detect_bom(bytes: &[u8]) -> (Option<String>, usize) {
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return (Some("UTF-8 BOM".to_string()), 3);
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return (Some("UTF-16 LE".to_string()), 2);
    }
    if bytes.starts_with(b"\xFE\xFF") {
        return (Some("UTF-16 BE".to_string()), 2);
    }
    if bytes.starts_with(b"\x00\x00\xFE\xFF") {
        return (Some("UTF-32 BE".to_string()), 4);
    }
    if bytes.starts_with(b"\xFF\xFE\x00\x00") {
        return (Some("UTF-32 LE".to_string()), 4);
    }
    (None, 0)
}

fn detect_delimiter(lines: &[&str]) -> u8 {
    let candidates = [b',', b';', b'\t', b'|'];
    let mut best = b',';
    let mut best_score = 0usize;
    for &delim in &candidates {
        let mut score = 0usize;
        for line in lines {
            score += line.as_bytes().iter().filter(|&&b| b == delim).count();
        }
        if score > best_score {
            best_score = score;
            best = delim;
        }
    }
    best
}

fn detect_quote(sample: &str) -> Option<String> {
    if sample.contains('"') {
        Some("\"".to_string())
    } else if sample.contains('\'') {
        Some("'".to_string())
    } else {
        None
    }
}

#[derive(Clone, Debug)]
enum ValueKind {
    Bool,
    Int,
    Float,
    Date,
    String,
}

struct ColumnStat {
    kind: ValueKind,
    nulls: usize,
}

fn ensure_stats(stats: &mut Vec<ColumnStat>, len: usize) {
    while stats.len() < len {
        stats.push(ColumnStat {
            kind: ValueKind::Bool,
            nulls: 0,
        });
    }
}

fn process_record(record: &csv::StringRecord, stats: &mut Vec<ColumnStat>) {
    for (index, value) in record.iter().enumerate() {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            stats[index].nulls += 1;
            continue;
        }
        let kind = infer_kind(trimmed);
        stats[index].kind = merge_kind(&stats[index].kind, &kind);
    }
}

fn infer_kind(value: &str) -> ValueKind {
    let lower = value.to_lowercase();
    if lower == "true" || lower == "false" || lower == "si" || lower == "no" {
        return ValueKind::Bool;
    }
    if value.parse::<i64>().is_ok() {
        return ValueKind::Int;
    }
    if value.parse::<f64>().is_ok() {
        return ValueKind::Float;
    }
    if looks_like_date(value) {
        return ValueKind::Date;
    }
    ValueKind::String
}

fn merge_kind(current: &ValueKind, new: &ValueKind) -> ValueKind {
    match (current, new) {
        (ValueKind::String, _) | (_, ValueKind::String) => ValueKind::String,
        (ValueKind::Float, ValueKind::Int) | (ValueKind::Int, ValueKind::Float) => ValueKind::Float,
        (ValueKind::Date, ValueKind::Date) => ValueKind::Date,
        (ValueKind::Bool, ValueKind::Bool) => ValueKind::Bool,
        (ValueKind::Int, ValueKind::Int) => ValueKind::Int,
        _ => ValueKind::String,
    }
}

fn looks_like_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' {
        return true;
    }
    if bytes.len() == 10 && (bytes[2] == b'/' || bytes[2] == b'-') {
        return true;
    }
    false
}

fn guess_header(first: &csv::StringRecord, second: &csv::StringRecord) -> bool {
    let first_numeric = first.iter().filter(|v| is_numeric(v)).count();
    let second_numeric = second.iter().filter(|v| is_numeric(v)).count();
    first_numeric < second_numeric
}

fn is_numeric(value: &str) -> bool {
    value.trim().parse::<f64>().is_ok()
}
