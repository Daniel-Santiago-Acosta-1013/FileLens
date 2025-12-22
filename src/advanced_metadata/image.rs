//! Extracción de metadata de imágenes (EXIF + detección XMP/IPTC).

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use exif::Tag;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const SIDECAR_SCAN_LIMIT: u64 = 2 * 1024 * 1024; // 2 MiB

pub fn extract_image_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata de imagen");
    let mut risks = Vec::new();

    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            section.notice = Some(SectionNotice::new(
                "No se pudo leer metadata de esta imagen",
                EntryLevel::Warning,
            ));
            return AdvancedMetadataResult { section, risks };
        }
    };

    let mut has_entries = false;

    let mut bufreader = BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut bufreader).ok();

    if let Some(exif) = exif {
        has_entries |= push_exif_value(&mut section, &exif, Tag::Make, "Fabricante");
        has_entries |= push_exif_value(&mut section, &exif, Tag::Model, "Modelo");
        has_entries |= push_exif_value(&mut section, &exif, Tag::Software, "Software");
        has_entries |= push_exif_value(&mut section, &exif, Tag::DateTime, "Fecha/Hora");
        has_entries |= push_exif_value(&mut section, &exif, Tag::FNumber, "Apertura");
        has_entries |= push_exif_value(&mut section, &exif, Tag::ExposureTime, "Exposición");
        has_entries |= push_exif_value(&mut section, &exif, Tag::PhotographicSensitivity, "ISO");
        has_entries |= push_exif_value(&mut section, &exif, Tag::FocalLength, "Distancia focal");
        has_entries |= push_exif_value(&mut section, &exif, Tag::Orientation, "Orientación");
        has_entries |= push_exif_value(&mut section, &exif, Tag::PixelXDimension, "Ancho");
        has_entries |= push_exif_value(&mut section, &exif, Tag::PixelYDimension, "Alto");
        has_entries |= push_exif_value(&mut section, &exif, Tag::Copyright, "Copyright");

        if let Some(field) = exif.get_field(Tag::GPSLatitude, exif::In::PRIMARY)
            && let Some(ref_field) = exif.get_field(Tag::GPSLatitudeRef, exif::In::PRIMARY)
        {
            let value = format!("{} {}", field.display_value(), ref_field.display_value());
            section.entries.push(ReportEntry::warning("GPS Latitud", &value));
            risks.push(ReportEntry::warning("GPS Latitud", value));
            has_entries = true;
        }

        if let Some(field) = exif.get_field(Tag::GPSLongitude, exif::In::PRIMARY)
            && let Some(ref_field) = exif.get_field(Tag::GPSLongitudeRef, exif::In::PRIMARY)
        {
            let value = format!("{} {}", field.display_value(), ref_field.display_value());
            section.entries.push(ReportEntry::warning("GPS Longitud", &value));
            risks.push(ReportEntry::warning("GPS Longitud", value));
            has_entries = true;
        }

        if let Some(field) = exif.get_field(Tag::GPSAltitude, exif::In::PRIMARY) {
            let value = field.display_value().to_string();
            section
                .entries
                .push(ReportEntry::warning("GPS Altitud", &value));
            risks.push(ReportEntry::warning("GPS Altitud", value));
            has_entries = true;
        }

        if let Some(field) = exif.get_field(Tag::Artist, exif::In::PRIMARY) {
            let value = field.display_value().to_string();
            section.entries.push(ReportEntry::warning("Artista", &value));
            risks.push(ReportEntry::warning("Artista", value));
            has_entries = true;
        }
    }

    let sidecar = detect_xmp_iptc(path);
    if sidecar.xmp {
        section
            .entries
            .push(ReportEntry::warning("XMP", "Detectado"));
        risks.push(ReportEntry::warning(
            "XMP embebido",
            "Puede contener metadata adicional",
        ));
        has_entries = true;
    }
    if sidecar.iptc {
        section
            .entries
            .push(ReportEntry::warning("IPTC", "Detectado"));
        risks.push(ReportEntry::warning(
            "IPTC embebido",
            "Puede contener metadata adicional",
        ));
        has_entries = true;
    }

    if !has_entries {
        section.notice = Some(SectionNotice::new(
            "No se encontró metadata EXIF/XMP/IPTC en esta imagen",
            EntryLevel::Muted,
        ));
    } else if !risks.is_empty() {
        section.notice = Some(SectionNotice::new(
            "⚠  Esta imagen contiene metadata que puede revelar información sensible",
            EntryLevel::Warning,
        ));
    }

    AdvancedMetadataResult { section, risks }
}

fn push_exif_value(section: &mut ReportSection, exif: &exif::Exif, tag: Tag, label: &str) -> bool {
    if let Some(field) = exif.get_field(tag, exif::In::PRIMARY) {
        section.entries.push(ReportEntry::info(
            label,
            field.display_value().to_string(),
        ));
        return true;
    }
    false
}

struct SidecarDetection {
    xmp: bool,
    iptc: bool,
}

fn detect_xmp_iptc(path: &Path) -> SidecarDetection {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return SidecarDetection {
                xmp: false,
                iptc: false,
            }
        }
    };

    let mut buffer = Vec::new();
    if file
        .take(SIDECAR_SCAN_LIMIT)
        .read_to_end(&mut buffer)
        .is_err()
    {
        return SidecarDetection {
            xmp: false,
            iptc: false,
        };
    }

    let xmp = contains_bytes(&buffer, b"<x:xmpmeta")
        || contains_bytes(&buffer, b"<?xpacket")
        || contains_bytes(&buffer, b"http://ns.adobe.com/xap/1.0/");

    let iptc = contains_bytes(&buffer, b"Photoshop 3.0")
        && contains_bytes(&buffer, b"8BIM")
        && contains_bytes(&buffer, b"IPTC");

    SidecarDetection { xmp, iptc }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
