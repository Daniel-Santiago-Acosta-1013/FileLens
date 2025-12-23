//! Extracción de metadata de imágenes (EXIF, PNG, XMP/IPTC).

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use exif::{In, Tag};
use image::ImageReader;
use png::text_metadata::{ITXtChunk, ZTXtChunk};
use png::Decoder as PngDecoder;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use xmltree::{Element, XMLNode};

use super::icc::extract_icc_profile;
use super::xmp::parse_xmp_metadata;

const SIDECAR_SCAN_LIMIT: u64 = 2 * 1024 * 1024; // 2 MiB
const TEXT_DECOMPRESS_LIMIT: usize = 2 * 1024 * 1024; // 2 MiB
const IFD_EXIF: In = In(2);
const IFD_GPS: In = In(3);
const IFD_INTEROP: In = In(4);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageKind {
    Jpeg,
    Png,
    Gif,
    Webp,
    Tiff,
    Heif,
    Svg,
    Unknown,
}

fn read_magic_bytes(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buffer = vec![0_u8; limit];
    let bytes_read = file.read(&mut buffer).ok()?;
    buffer.truncate(bytes_read);
    Some(buffer)
}

fn detect_image_kind(path: &Path) -> ImageKind {
    let Some(prefix) = read_magic_bytes(path, 256) else {
        return ImageKind::Unknown;
    };
    if prefix.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ImageKind::Jpeg;
    }
    if prefix.starts_with(b"\x89PNG\r\n\x1a\n") {
        return ImageKind::Png;
    }
    if prefix.starts_with(b"GIF87a") || prefix.starts_with(b"GIF89a") {
        return ImageKind::Gif;
    }
    if prefix.len() >= 12 && &prefix[0..4] == b"RIFF" && &prefix[8..12] == b"WEBP" {
        return ImageKind::Webp;
    }
    if prefix.starts_with(b"II*\0") || prefix.starts_with(b"MM\0*") || prefix.starts_with(b"II+\0")
    {
        return ImageKind::Tiff;
    }
    if prefix.len() >= 12 && &prefix[4..8] == b"ftyp" {
        let brand = &prefix[8..12];
        if matches!(
            brand,
            b"heic" | b"heif" | b"heix" | b"mif1" | b"msf1" | b"avif"
        ) {
            return ImageKind::Heif;
        }
    }
    let prefix_str = String::from_utf8_lossy(&prefix).to_lowercase();
    if prefix_str.contains("<svg") {
        return ImageKind::Svg;
    }
    ImageKind::Unknown
}

pub fn extract_image_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata de imagen");
    let mut risks = Vec::new();
    let mut seen = HashSet::new();

    let mut has_entries = false;
    let mut xmp_detected = false;
    let mut xmp_parsed = false;
    let kind = detect_image_kind(path);

    if !matches!(kind, ImageKind::Svg) {
        if let Some(exif) = read_exif(path) {
            has_entries |= append_exif_entries(&mut section, &mut risks, &mut seen, &exif);
        }
    }

    let mut dimensions = None;

    match kind {
        ImageKind::Jpeg => {
            if let Some(jpeg) = read_jpeg_metadata(path) {
                dimensions = jpeg.dimensions;
                has_entries |= append_jpeg_entries(&mut section, &mut risks, &mut seen, &jpeg);

                if let Some(profile) = jpeg.icc_profile {
                    has_entries |= push_entry_unique(
                        &mut section,
                        &mut seen,
                        ReportEntry::info("Perfil ICC", format!("{} bytes", profile.len())),
                    );
                    let icc_entries = extract_icc_profile(&profile);
                    for entry in icc_entries {
                        has_entries |= push_entry_unique(&mut section, &mut seen, entry);
                    }
                }
            }
        }
        ImageKind::Png => {
            if let Some(png) = read_png_metadata(path) {
                dimensions = Some((png.width, png.height));
                has_entries |= append_png_entries(&mut section, &mut risks, &mut seen, &png);

                if let Some(profile) = png.icc_profile {
                    has_entries |= push_entry_unique(
                        &mut section,
                        &mut seen,
                        ReportEntry::info("Perfil ICC", format!("{} bytes", profile.len())),
                    );
                    let icc_entries = extract_icc_profile(&profile);
                    for entry in icc_entries {
                        has_entries |= push_entry_unique(&mut section, &mut seen, entry);
                    }
                }

                if let Some(xmp) = png.xmp_packet {
                    xmp_detected = true;
                    xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
                }
            }
        }
        ImageKind::Gif => {
            if let Some(gif) = read_gif_metadata(path) {
                dimensions = Some((gif.width, gif.height));
                has_entries |= append_gif_entries(&mut section, &mut risks, &mut seen, &gif);
            }
        }
        ImageKind::Webp => {
            if let Some(webp) = read_webp_metadata(path) {
                dimensions = webp.dimensions;
                has_entries |= append_webp_entries(&mut section, &mut risks, &mut seen, &webp);
                if let Some(profile) = webp.icc_profile {
                    has_entries |= push_entry_unique(
                        &mut section,
                        &mut seen,
                        ReportEntry::info("Perfil ICC", format!("{} bytes", profile.len())),
                    );
                    let icc_entries = extract_icc_profile(&profile);
                    for entry in icc_entries {
                        has_entries |= push_entry_unique(&mut section, &mut seen, entry);
                    }
                }
                if let Some(xmp) = webp.xmp_packet {
                    xmp_detected = true;
                    xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
                }
            }
        }
        ImageKind::Tiff => {
            if let Some(tiff) = read_tiff_metadata(path) {
                dimensions = tiff.dimensions;
                has_entries |= append_tiff_entries(&mut section, &mut risks, &mut seen, &tiff);
                if let Some(profile) = tiff.icc_profile {
                    has_entries |= push_entry_unique(
                        &mut section,
                        &mut seen,
                        ReportEntry::info("Perfil ICC", format!("{} bytes", profile.len())),
                    );
                    let icc_entries = extract_icc_profile(&profile);
                    for entry in icc_entries {
                        has_entries |= push_entry_unique(&mut section, &mut seen, entry);
                    }
                }
                if let Some(xmp) = tiff.xmp_packet {
                    xmp_detected = true;
                    xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
                }
            }
        }
        ImageKind::Heif => {
            if let Some(heif) = read_heif_metadata(path) {
                dimensions = heif.dimensions;
                has_entries |= append_heif_entries(&mut section, &mut risks, &mut seen, &heif);
                if let Some(profile) = heif.icc_profile {
                    has_entries |= push_entry_unique(
                        &mut section,
                        &mut seen,
                        ReportEntry::info("Perfil ICC", format!("{} bytes", profile.len())),
                    );
                    let icc_entries = extract_icc_profile(&profile);
                    for entry in icc_entries {
                        has_entries |= push_entry_unique(&mut section, &mut seen, entry);
                    }
                }
                if let Some(xmp) = heif.xmp_packet {
                    xmp_detected = true;
                    xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
                }
            }
        }
        ImageKind::Svg => {
            if let Some(svg) = read_svg_metadata(path) {
                dimensions = svg.dimensions;
                has_entries |= append_svg_entries(&mut section, &mut risks, &mut seen, &svg);
                if let Some(xmp) = svg.xmp_packet {
                    xmp_detected = true;
                    xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
                }
            }
        }
        ImageKind::Unknown => {}
    }

    if dimensions.is_none() {
        dimensions = read_image_dimensions(path);
    }

    if let Some((width, height)) = dimensions {
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::info("Ancho", width.to_string()),
        );
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::info("Alto", height.to_string()),
        );
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::info("Tamaño de imagen", format!("{}x{}", width, height)),
        );
        let megapixels = (width as f64 * height as f64) / 1_000_000.0;
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::info("Megapíxeles", format!("{megapixels:.3}")),
        );
    }

    if !xmp_detected && let Some(xmp) = scan_xmp_packet(path) {
        xmp_detected = true;
        xmp_parsed |= append_xmp_entries(&mut section, &mut risks, &mut seen, &xmp);
    }

    if xmp_detected && !xmp_parsed {
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::warning("XMP", "Detectado"),
        );
        risks.push(ReportEntry::warning(
            "XMP embebido",
            "Puede contener metadata adicional",
        ));
    }

    if let Some(iptc) = extract_iptc_metadata(path) {
        has_entries |= append_iptc_entries(&mut section, &mut risks, &mut seen, &iptc);
    } else if detect_iptc(path) {
        has_entries |= push_entry_unique(
            &mut section,
            &mut seen,
            ReportEntry::warning("IPTC", "Detectado"),
        );
        risks.push(ReportEntry::warning(
            "IPTC embebido",
            "Puede contener metadata adicional",
        ));
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

fn read_exif(path: &Path) -> Option<exif::Exif> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    exif::Reader::new().read_from_container(&mut bufreader).ok()
}

fn append_exif_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    exif: &exif::Exif,
) -> bool {
    let mut has_entries = false;
    let gps_lat = gps_dms_from_exif(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef);
    let gps_lon = gps_dms_from_exif(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef);
    let byte_order = if exif.little_endian() {
        "Little-endian (Intel, II)"
    } else {
        "Big-endian (Motorola, MM)"
    };
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Orden de bytes EXIF", byte_order),
    );

    let specs = [
        ExifSpec::info(Tag::Make, "Fabricante"),
        ExifSpec::info(Tag::Model, "Modelo"),
        ExifSpec::warning(Tag::Artist, "Artista"),
        ExifSpec::warning(Tag::Software, "Software"),
        ExifSpec::warning(Tag::Copyright, "Copyright"),
        ExifSpec::warning(Tag::UserComment, "Comentario de usuario"),
        ExifSpec::info(Tag::ImageDescription, "Descripción"),
        ExifSpec::info(Tag::DateTime, "Fecha/Hora"),
        ExifSpec::info(Tag::DateTimeOriginal, "Fecha/Hora original"),
        ExifSpec::info(Tag::DateTimeDigitized, "Fecha/Hora digitalización"),
        ExifSpec::info(Tag::OffsetTime, "Zona horaria"),
        ExifSpec::info(Tag::OffsetTimeOriginal, "Zona horaria original"),
        ExifSpec::info(Tag::OffsetTimeDigitized, "Zona horaria digitalización"),
        ExifSpec::info(Tag::FNumber, "Apertura"),
        ExifSpec::info(Tag::ExposureTime, "Exposición"),
        ExifSpec::info(Tag::ShutterSpeedValue, "Velocidad de obturación"),
        ExifSpec::info(Tag::ExposureProgram, "Programa de exposición"),
        ExifSpec::info(Tag::ExposureMode, "Modo de exposición"),
        ExifSpec::info(Tag::ExposureBiasValue, "Compensación de exposición"),
        ExifSpec::info(Tag::PhotographicSensitivity, "ISO"),
        ExifSpec::info(Tag::ISOSpeed, "ISO"),
        ExifSpec::info(Tag::RecommendedExposureIndex, "ISO recomendado"),
        ExifSpec::info(Tag::FocalLength, "Distancia focal"),
        ExifSpec::info(Tag::LensSpecification, "Especificación de lente"),
        ExifSpec::info(Tag::Orientation, "Orientación"),
        ExifSpec::info(Tag::XResolution, "Resolución X"),
        ExifSpec::info(Tag::YResolution, "Resolución Y"),
        ExifSpec::info(Tag::ResolutionUnit, "Unidad de resolución"),
        ExifSpec::info(Tag::Flash, "Flash"),
        ExifSpec::info(Tag::WhiteBalance, "Balance de blancos"),
        ExifSpec::info(Tag::MeteringMode, "Modo de medición"),
        ExifSpec::info(Tag::LensMake, "Fabricante de lente"),
        ExifSpec::info(Tag::LensModel, "Modelo de lente"),
        ExifSpec::warning(Tag::LensSerialNumber, "Número de serie de lente"),
        ExifSpec::warning(Tag::BodySerialNumber, "Número de serie"),
        ExifSpec::warning(Tag::CameraOwnerName, "Propietario de cámara"),
    ];

    for spec in specs {
        if let Some(field) = get_exif_field(exif, spec.tag) {
            let value = field.display_value().with_unit(exif).to_string();
            let entry = ReportEntry::new(spec.label, &value, spec.level);
            if push_entry_unique(section, seen, entry) {
                has_entries = true;
                if spec.level == EntryLevel::Warning {
                    risks.push(ReportEntry::warning(spec.label, value));
                }
            }
        }
    }

    if let (Some(lat), Some(lon)) = (&gps_lat, &gps_lon) {
        let position = format!("{}, {}", format_gps_dms(lat), format_gps_dms(lon));
        if push_entry_unique(
            section,
            seen,
            ReportEntry::warning("Posición GPS", &position),
        ) {
            risks.push(ReportEntry::warning("Posición GPS", position));
            has_entries = true;
        }
    }

    if let Some(lat) = gps_lat {
        let value = format_gps_dms(&lat);
        if push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Latitud", &value),
        ) {
            risks.push(ReportEntry::warning("GPS Latitud", value));
            has_entries = true;
        }
    } else if let Some(value) = gps_value(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef)
        && push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Latitud", &value),
        )
    {
        risks.push(ReportEntry::warning("GPS Latitud", value));
        has_entries = true;
    }

    if let Some(lon) = gps_lon {
        let value = format_gps_dms(&lon);
        if push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Longitud", &value),
        ) {
            risks.push(ReportEntry::warning("GPS Longitud", value));
            has_entries = true;
        }
    } else if let Some(value) = gps_value(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef)
        && push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Longitud", &value),
        )
    {
        risks.push(ReportEntry::warning("GPS Longitud", value));
        has_entries = true;
    }
    if let Some(field) = exif.get_field(Tag::GPSAltitude, IFD_GPS) {
        let value = field.display_value().to_string();
        if push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Altitud", &value),
        ) {
            risks.push(ReportEntry::warning("GPS Altitud", value));
            has_entries = true;
        }
    }

    if let Some(value) = gps_value(exif, Tag::GPSSpeed, Tag::GPSSpeedRef)
        && push_entry_unique(section, seen, ReportEntry::warning("GPS Velocidad", &value))
    {
        risks.push(ReportEntry::warning("GPS Velocidad", value));
        has_entries = true;
    }

    if let Some(value) = gps_value(exif, Tag::GPSTrack, Tag::GPSTrackRef)
        && push_entry_unique(section, seen, ReportEntry::warning("GPS Rumbo", &value))
    {
        risks.push(ReportEntry::warning("GPS Rumbo", value));
        has_entries = true;
    }

    if let Some(value) = gps_value(exif, Tag::GPSImgDirection, Tag::GPSImgDirectionRef)
        && push_entry_unique(section, seen, ReportEntry::warning("GPS Dirección", &value))
    {
        risks.push(ReportEntry::warning("GPS Dirección", value));
        has_entries = true;
    }

    if let Some(field) = exif.get_field(Tag::GPSMapDatum, IFD_GPS) {
        let value = field.display_value().to_string();
        if push_entry_unique(section, seen, ReportEntry::warning("GPS Datum", &value)) {
            risks.push(ReportEntry::warning("GPS Datum", value));
            has_entries = true;
        }
    }

    has_entries
}

fn get_exif_field(exif: &exif::Exif, tag: Tag) -> Option<&exif::Field> {
    for ifd in [In::PRIMARY, IFD_EXIF, IFD_GPS, IFD_INTEROP] {
        if let Some(field) = exif.get_field(tag, ifd) {
            return Some(field);
        }
    }
    None
}

fn gps_value(exif: &exif::Exif, value_tag: Tag, ref_tag: Tag) -> Option<String> {
    let field = exif.get_field(value_tag, IFD_GPS)?;
    let value = field.display_value().to_string();
    if let Some(ref_field) = exif.get_field(ref_tag, IFD_GPS) {
        Some(format!("{} {}", value, ref_field.display_value()))
    } else {
        Some(value)
    }
}

struct GpsDms {
    degrees: f64,
    minutes: f64,
    seconds: f64,
    reference: Option<char>,
}

fn gps_dms_from_exif(
    exif: &exif::Exif,
    value_tag: Tag,
    ref_tag: Tag,
) -> Option<GpsDms> {
    use exif::Value;

    let field = exif.get_field(value_tag, IFD_GPS)?;
    let (degrees, minutes, seconds) = match &field.value {
        Value::Rational(values) => gps_rational_triplet(values)?,
        Value::SRational(values) => gps_srational_triplet(values)?,
        _ => return None,
    };
    let reference = exif
        .get_field(ref_tag, IFD_GPS)
        .and_then(|field| gps_ref_char(&field.display_value().to_string()));

    Some(GpsDms {
        degrees,
        minutes,
        seconds,
        reference,
    })
}

fn gps_rational_triplet(values: &[exif::Rational]) -> Option<(f64, f64, f64)> {
    if values.len() < 3 {
        return None;
    }
    let degrees = values[0].num as f64 / values[0].denom as f64;
    let minutes = values[1].num as f64 / values[1].denom as f64;
    let seconds = values[2].num as f64 / values[2].denom as f64;
    Some((degrees, minutes, seconds))
}

fn gps_srational_triplet(values: &[exif::SRational]) -> Option<(f64, f64, f64)> {
    if values.len() < 3 {
        return None;
    }
    let degrees = values[0].num as f64 / values[0].denom as f64;
    let minutes = values[1].num as f64 / values[1].denom as f64;
    let seconds = values[2].num as f64 / values[2].denom as f64;
    Some((degrees, minutes, seconds))
}

fn gps_ref_char(value: &str) -> Option<char> {
    value
        .chars()
        .find_map(|ch| match ch.to_ascii_uppercase() {
            'N' | 'S' | 'E' | 'W' => Some(ch.to_ascii_uppercase()),
            _ => None,
        })
}

fn format_gps_dms(coord: &GpsDms) -> String {
    let (degrees, minutes, seconds) = normalize_dms(coord.degrees, coord.minutes, coord.seconds);
    let deg_label = format_decimal(degrees.abs(), 0);
    let min_label = format_decimal(minutes.abs(), 0);
    let sec_label = format_decimal(seconds.abs(), 2);
    let reference = coord
        .reference
        .map(|c| format!(" {}", c))
        .unwrap_or_default();
    format!("{deg_label} grados {min_label}' {sec_label}\"{reference}")
}

fn normalize_dms(degrees: f64, minutes: f64, seconds: f64) -> (f64, f64, f64) {
    let mut deg = degrees;
    let mut min = minutes;
    let mut sec = seconds;
    if sec >= 60.0 {
        min += (sec / 60.0).floor();
        sec = sec % 60.0;
    }
    if min >= 60.0 {
        deg += (min / 60.0).floor();
        min = min % 60.0;
    }
    (deg, min, sec)
}

fn format_decimal(value: f64, decimals: usize) -> String {
    let mut out = format!("{:.*}", decimals, value);
    if out.contains('.') {
        out = out.replace('.', ",");
    }
    out
}

fn read_image_dimensions(path: &Path) -> Option<(u32, u32)> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    reader.into_dimensions().ok()
}

fn append_png_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    png: &PngMetadata,
) -> bool {
    let mut has_entries = false;

    let bit_depth = match png.bit_depth {
        png::BitDepth::One => "1",
        png::BitDepth::Two => "2",
        png::BitDepth::Four => "4",
        png::BitDepth::Eight => "8",
        png::BitDepth::Sixteen => "16",
    };
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Profundidad de bits", bit_depth),
    );

    let color_type = match png.color_type {
        png::ColorType::Grayscale => "Escala de grises",
        png::ColorType::Rgb => "RGB",
        png::ColorType::Indexed => "Indexado",
        png::ColorType::GrayscaleAlpha => "Gris con alfa",
        png::ColorType::Rgba => "RGB con Alfa",
    };
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Tipo de color", color_type),
    );

    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Compresión", "Desinflar/Inflar"),
    );
    has_entries |= push_entry_unique(section, seen, ReportEntry::info("Filtrar", "Adaptado"));

    let interlace_label = if png.interlaced {
        "Entrelazado (Adam7)"
    } else {
        "No entrelazado"
    };
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Entrelazado", interlace_label),
    );

    if !png.chunk_list.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Chunks presentes", png.chunk_list.join(", ")),
        );
    }

    if !png.chunk_counts.is_empty() {
        let mut counts = png
            .chunk_counts
            .iter()
            .map(|(key, value)| format!("{key}:{value}"))
            .collect::<Vec<_>>();
        counts.sort();
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Chunks por tipo", counts.join(", ")),
        );
    }

    if png.text_bytes > 0 {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Tamaño total de texto", format!("{} bytes", png.text_bytes)),
        );
    }

    if let Some(time) = &png.time {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Fecha/hora interna", time),
        );
    }

    if let Some(phys) = &png.phys {
        let unit = match phys.unit {
            1 => "px/m",
            _ => "sin unidad",
        };
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Densidad física",
                format!("{} x {} {}", phys.x, phys.y, unit),
            ),
        );
    }

    if let Some(gamma) = png.gamma {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Gamma", format!("{gamma:.5}")),
        );
    }

    if let Some(intent) = png.srgb_intent {
        let intent_label = match intent {
            png::SrgbRenderingIntent::Perceptual => "Perceptivo",
            png::SrgbRenderingIntent::RelativeColorimetric => "Colorimétrico relativo",
            png::SrgbRenderingIntent::Saturation => "Saturación",
            png::SrgbRenderingIntent::AbsoluteColorimetric => "Colorimétrico absoluto",
        };
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("sRGB", intent_label),
        );
    }

    if let Some(name) = &png.icc_name {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Perfil ICC (nombre)", name),
        );
    }

    if let Some(chroma) = &png.chromaticities {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Cromaticidades", chroma),
        );
    }

    for chunk in &png.text_chunks {
        if let Some((label, sensitive)) = map_png_text_label(&chunk.keyword) {
            let level = if sensitive {
                EntryLevel::Warning
            } else {
                EntryLevel::Info
            };
            let entry = ReportEntry::new(label, &chunk.text, level);
            if push_entry_unique(section, seen, entry) {
                has_entries = true;
                if sensitive {
                    risks.push(ReportEntry::warning(label, chunk.text.clone()));
                }
            }
            continue;
        }

        let label = if let Some(lang) = &chunk.language {
            format!("Texto {} ({}, {})", chunk.chunk_type, chunk.keyword, lang)
        } else {
            format!("Texto {} ({})", chunk.chunk_type, chunk.keyword)
        };
        if push_entry_unique(section, seen, ReportEntry::info(&label, &chunk.text)) {
            has_entries = true;
        }
    }

    has_entries
}

fn map_png_text_label(keyword: &str) -> Option<(&'static str, bool)> {
    match keyword.to_lowercase().as_str() {
        "title" => Some(("Título", false)),
        "description" => Some(("Descripción", false)),
        "author" => Some(("Autor", true)),
        "creator" => Some(("Creador", true)),
        "copyright" => Some(("Copyright", true)),
        "comment" => Some(("Comentario de usuario", true)),
        "software" => Some(("Software", true)),
        "creation time" => Some(("Fecha de creación", false)),
        "source" => Some(("Fuente", false)),
        _ => None,
    }
}

fn append_xmp_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    xmp: &str,
) -> bool {
    let Some(metadata) = parse_xmp_metadata(xmp) else {
        return false;
    };
    let mut has_entries = false;
    if let Some(position) = metadata.gps_position {
        if push_entry_unique(
            section,
            seen,
            ReportEntry::warning("Posición GPS", &position),
        ) {
            risks.push(ReportEntry::warning("Posición GPS", position));
            has_entries = true;
        }
    }
    for entry in metadata.entries {
        has_entries |= push_entry_unique(section, seen, entry);
    }
    for risk in metadata.risks {
        risks.push(risk);
    }
    has_entries
}

fn read_png_metadata(path: &Path) -> Option<PngMetadata> {
    let file = File::open(path).ok()?;
    let decoder = PngDecoder::new(BufReader::new(file));
    let reader = decoder.read_info().ok()?;
    let info = reader.info();

    let scan = scan_png_chunks(path);

    let mut text_chunks = Vec::new();
    let mut xmp_packet = None;

    for chunk in &info.uncompressed_latin1_text {
        let keyword = chunk.keyword.clone();
        let text = chunk.text.clone();
        if is_xmp_keyword(&keyword) {
            xmp_packet = Some(text);
            continue;
        }
        text_chunks.push(TextChunk {
            keyword,
            text,
            chunk_type: "tEXt",
            language: None,
        });
    }

    for chunk in &info.compressed_latin1_text {
        let keyword = chunk.keyword.clone();
        if let Some(text) = decode_ztxt(chunk) {
            if is_xmp_keyword(&keyword) {
                xmp_packet = Some(text);
                continue;
            }
            text_chunks.push(TextChunk {
                keyword,
                text,
                chunk_type: "zTXt",
                language: None,
            });
        }
    }

    for chunk in &info.utf8_text {
        let keyword = chunk.keyword.clone();
        if let Some(text) = decode_itxt(chunk) {
            if is_xmp_keyword(&keyword) {
                xmp_packet = Some(text);
                continue;
            }
            let language = if chunk.language_tag.trim().is_empty() {
                None
            } else {
                Some(chunk.language_tag.clone())
            };
            text_chunks.push(TextChunk {
                keyword,
                text,
                chunk_type: "iTXt",
                language,
            });
        }
    }

    let (chunk_list, chunk_counts, text_bytes, icc_name, chromaticities, phys, time) =
        if let Some(scan) = scan {
            (
                scan.chunk_list,
                scan.chunk_counts,
                scan.text_bytes,
                scan.icc_name,
                scan.chromaticities,
                scan.phys,
                scan.time,
            )
        } else {
            (
                Vec::new(),
                HashMap::new(),
                0,
                None,
                None,
                None,
                None,
            )
        };

    Some(PngMetadata {
        width: info.width,
        height: info.height,
        bit_depth: info.bit_depth,
        color_type: info.color_type,
        interlaced: info.interlaced,
        gamma: info
            .source_gamma
            .map(|gamma: png::ScaledFloat| gamma.into_value()),
        srgb_intent: info.srgb,
        icc_profile: info
            .icc_profile
            .as_ref()
            .map(|data| data.as_ref().to_vec()),
        icc_name,
        chromaticities,
        phys,
        chunk_list,
        chunk_counts,
        text_bytes,
        text_chunks,
        xmp_packet,
        time,
    })
}

struct PngChunkScan {
    chunk_list: Vec<String>,
    chunk_counts: HashMap<String, usize>,
    text_bytes: usize,
    icc_name: Option<String>,
    chromaticities: Option<String>,
    phys: Option<PngPhys>,
    time: Option<String>,
}

fn scan_png_chunks(path: &Path) -> Option<PngChunkScan> {
    let mut file = File::open(path).ok()?;
    let mut signature = [0_u8; 8];
    file.read_exact(&mut signature).ok()?;
    if signature != *b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    let mut chunk_list = Vec::new();
    let mut chunk_counts: HashMap<String, usize> = HashMap::new();
    let mut seen = HashSet::new();
    let mut text_bytes: usize = 0;
    let mut icc_name = None;
    let mut chromaticities = None;
    let mut phys = None;
    let mut time = None;

    loop {
        let length = match read_u32_be_from(&mut file) {
            Some(value) => value as usize,
            None => break,
        };
        let mut chunk_type = [0_u8; 4];
        if file.read_exact(&mut chunk_type).is_err() {
            break;
        }
        let chunk_name = String::from_utf8_lossy(&chunk_type).to_string();
        *chunk_counts.entry(chunk_name.clone()).or_insert(0) += 1;
        if seen.insert(chunk_name.clone()) {
            chunk_list.push(chunk_name.clone());
        }
        if matches!(chunk_name.as_str(), "tEXt" | "zTXt" | "iTXt") {
            text_bytes = text_bytes.saturating_add(length);
        }

        let needs_payload = matches!(
            chunk_name.as_str(),
            "tIME" | "pHYs" | "cHRM" | "iCCP"
        );
        if needs_payload {
            let mut payload = vec![0_u8; length];
            if file.read_exact(&mut payload).is_err() {
                break;
            }
            match chunk_name.as_str() {
                "tIME" if payload.len() >= 7 => {
                    let year = u16::from_be_bytes([payload[0], payload[1]]);
                    let month = payload[2];
                    let day = payload[3];
                    let hour = payload[4];
                    let minute = payload[5];
                    let second = payload[6];
                    time = Some(format!(
                        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
                    ));
                }
                "pHYs" if payload.len() >= 9 => {
                    let x = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    let y = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                    let unit = payload[8];
                    phys = Some(PngPhys { x, y, unit });
                }
                "cHRM" if payload.len() >= 32 => {
                    let mut vals = Vec::new();
                    for i in 0..8 {
                        let start = i * 4;
                        let value = u32::from_be_bytes([
                            payload[start],
                            payload[start + 1],
                            payload[start + 2],
                            payload[start + 3],
                        ]);
                        vals.push(format!("{:.5}", value as f64 / 100_000.0));
                    }
                    chromaticities = Some(vals.join(", "));
                }
                "iCCP" if icc_name.is_none() => {
                    if let Some(null_pos) = payload.iter().position(|&b| b == 0) {
                        let name = String::from_utf8_lossy(&payload[..null_pos]).to_string();
                        if !name.trim().is_empty() {
                            icc_name = Some(name);
                        }
                    }
                }
                _ => {}
            }
        } else {
            if file.seek(SeekFrom::Current(length as i64)).is_err() {
                break;
            }
        }

        let mut crc = [0_u8; 4];
        if file.read_exact(&mut crc).is_err() {
            break;
        }
        if chunk_name == "IEND" {
            break;
        }
    }

    Some(PngChunkScan {
        chunk_list,
        chunk_counts,
        text_bytes,
        icc_name,
        chromaticities,
        phys,
        time,
    })
}

fn read_u32_be_from<R: Read>(reader: &mut R) -> Option<u32> {
    let mut buffer = [0_u8; 4];
    reader.read_exact(&mut buffer).ok()?;
    Some(u32::from_be_bytes(buffer))
}

fn read_u16_be_from<R: Read>(reader: &mut R) -> Option<u16> {
    let mut buffer = [0_u8; 2];
    reader.read_exact(&mut buffer).ok()?;
    Some(u16::from_be_bytes(buffer))
}

struct JpegMetadata {
    has_jfif: bool,
    has_exif: bool,
    jfif_version: Option<String>,
    density_units: Option<String>,
    x_density: Option<u16>,
    y_density: Option<u16>,
    comment: Option<String>,
    app_segments: Vec<String>,
    icc_profile: Option<Vec<u8>>,
    thumbnail: Option<String>,
    dimensions: Option<(u32, u32)>,
    bits_per_component: Option<u8>,
    components: Vec<JpegComponent>,
    mode: Option<&'static str>,
    adobe_transform: Option<u8>,
}

struct JpegComponent {
    _id: u8,
    h: u8,
    v: u8,
}

fn read_jpeg_metadata(path: &Path) -> Option<JpegMetadata> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut soi = [0_u8; 2];
    reader.read_exact(&mut soi).ok()?;
    if soi != [0xFF, 0xD8] {
        return None;
    }

    let mut app_segments = HashSet::new();
    let mut has_jfif = false;
    let mut has_exif = false;
    let mut jfif_version = None;
    let mut density_units = None;
    let mut x_density = None;
    let mut y_density = None;
    let mut thumbnail = None;
    let mut comment = None;
    let mut icc_total = 0_u8;
    let mut icc_chunks: Vec<Option<Vec<u8>>> = Vec::new();
    let mut dimensions = None;
    let mut bits_per_component = None;
    let mut components = Vec::new();
    let mut mode = None;
    let mut adobe_transform = None;

    while let Some(marker) = read_jpeg_marker(&mut reader) {
        if marker == 0xD9 {
            break;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let length = read_u16_be_from(&mut reader)? as usize;
        if length < 2 {
            break;
        }
        let data_len = length - 2;
        let mut data = vec![0_u8; data_len];
        if reader.read_exact(&mut data).is_err() {
            break;
        }

        if (0xE0..=0xEF).contains(&marker) {
            app_segments.insert(format!("APP{}", marker - 0xE0));
        }

        match marker {
            0xE0 => {
                if data.starts_with(b"JFIF\0") && data.len() >= 14 {
                    has_jfif = true;
                    jfif_version = Some(format!("{}.{}", data[5], data[6]));
                    let unit_label = match data[7] {
                        1 => "dpi",
                        2 => "dpcm",
                        _ => "sin unidad",
                    };
                    density_units = Some(unit_label.to_string());
                    x_density = Some(u16::from_be_bytes([data[8], data[9]]));
                    y_density = Some(u16::from_be_bytes([data[10], data[11]]));
                    let x_thumb = data[12] as u32;
                    let y_thumb = data[13] as u32;
                    if x_thumb > 0 && y_thumb > 0 {
                        let size = (x_thumb * y_thumb * 3) as usize;
                        thumbnail = Some(format!("{x_thumb}x{y_thumb} ({size} bytes)"));
                    }
                }
            }
            0xE1 => {
                if data.starts_with(b"Exif\0\0") {
                    has_exif = true;
                }
            }
            0xE2 => {
                if data.starts_with(b"ICC_PROFILE\0") && data.len() > 14 {
                    let seq = data[12] as usize;
                    let total = data[13] as usize;
                    if total > 0 {
                        icc_total = total as u8;
                        if icc_chunks.len() < total {
                            icc_chunks.resize_with(total, || None);
                        }
                        if seq > 0 && seq <= total {
                            icc_chunks[seq - 1] = Some(data[14..].to_vec());
                        }
                    }
                }
            }
            0xEE => {
                if data.starts_with(b"Adobe") && data.len() >= 12 {
                    adobe_transform = Some(data[11]);
                }
            }
            0xFE => {
                let value = String::from_utf8_lossy(&data).trim().to_string();
                if !value.is_empty() {
                    comment = Some(value);
                }
            }
            0xC0 | 0xC1 | 0xC2 | 0xC3 => {
                if data.len() >= 6 {
                    bits_per_component = Some(data[0]);
                    let height = u16::from_be_bytes([data[1], data[2]]) as u32;
                    let width = u16::from_be_bytes([data[3], data[4]]) as u32;
                    dimensions = Some((width, height));
                    mode = Some(match marker {
                        0xC0 => "Baseline",
                        0xC1 => "Extendido",
                        0xC2 => "Progresivo",
                        0xC3 => "Lossless",
                        _ => "Desconocido",
                    });
                    let component_count = data[5] as usize;
                    components.clear();
                    let mut offset = 6;
                    for _ in 0..component_count {
                        if offset + 2 >= data.len() {
                            break;
                        }
                        let id = data[offset];
                        let sampling = data[offset + 1];
                        let h = sampling >> 4;
                        let v = sampling & 0x0F;
                        components.push(JpegComponent { _id: id, h, v });
                        offset += 3;
                    }
                }
            }
            _ => {}
        }
    }

    let icc_profile = if icc_total > 0 && icc_chunks.iter().all(|part| part.is_some()) {
        let mut merged = Vec::new();
        for part in icc_chunks.into_iter().flatten() {
            merged.extend_from_slice(&part);
        }
        Some(merged)
    } else {
        None
    };

    let mut app_list = app_segments.into_iter().collect::<Vec<_>>();
    app_list.sort();

    Some(JpegMetadata {
        has_jfif,
        has_exif,
        jfif_version,
        density_units,
        x_density,
        y_density,
        comment,
        app_segments: app_list,
        icc_profile,
        thumbnail,
        dimensions,
        bits_per_component,
        components,
        mode,
        adobe_transform,
    })
}

fn append_jpeg_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    jpeg: &JpegMetadata,
) -> bool {
    let mut has_entries = false;
    let format = if jpeg.has_exif {
        "Exif JPEG"
    } else if jpeg.has_jfif {
        "JFIF"
    } else {
        "JPEG"
    };
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("JPEG Formato", format),
    );

    if let Some(version) = &jpeg.jfif_version {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("JFIF Versión", version),
        );
    }

    if let Some(units) = &jpeg.density_units {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Unidades de densidad", units),
        );
    }

    if let Some(value) = jpeg.x_density {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Densidad X", value.to_string()),
        );
    }
    if let Some(value) = jpeg.y_density {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Densidad Y", value.to_string()),
        );
    }
    if let (Some(x), Some(y), Some(units)) =
        (jpeg.x_density, jpeg.y_density, jpeg.density_units.as_deref())
    {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Resolución", format!("{x}x{y} {units}")),
        );
    }

    if let Some(comment) = &jpeg.comment {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::warning("Comentario JPEG", comment),
        );
        risks.push(ReportEntry::warning("Comentario JPEG", comment.to_string()));
    }

    if !jpeg.app_segments.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Segmentos APP", jpeg.app_segments.join(", ")),
        );
    }

    if let Some(bits) = jpeg.bits_per_component {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Bits por componente", bits.to_string()),
        );
    }

    if let Some(mode) = jpeg.mode {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Modo JPEG", mode));
    }

    if let Some(color) = jpeg_color_space(&jpeg.components, jpeg.adobe_transform) {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Espacio de color", color),
        );
    }

    if let Some(subsampling) = jpeg_subsampling(&jpeg.components) {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Submuestreo", subsampling),
        );
    }

    if let Some(thumbnail) = &jpeg.thumbnail {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Miniatura embebida", thumbnail),
        );
    }

    has_entries
}

fn read_jpeg_marker<R: Read>(reader: &mut R) -> Option<u8> {
    let mut byte = [0_u8; 1];
    loop {
        reader.read_exact(&mut byte).ok()?;
        if byte[0] == 0xFF {
            break;
        }
    }
    loop {
        reader.read_exact(&mut byte).ok()?;
        if byte[0] != 0xFF {
            return Some(byte[0]);
        }
    }
}

fn jpeg_color_space(components: &[JpegComponent], adobe_transform: Option<u8>) -> Option<&'static str> {
    match components.len() {
        1 => Some("Grayscale"),
        3 => match adobe_transform {
            Some(0) => Some("RGB"),
            Some(1) => Some("YCbCr"),
            _ => Some("YCbCr"),
        },
        4 => match adobe_transform {
            Some(2) => Some("YCCK"),
            _ => Some("CMYK"),
        },
        _ => None,
    }
}

fn jpeg_subsampling(components: &[JpegComponent]) -> Option<String> {
    if components.len() < 3 {
        return None;
    }
    let y = &components[0];
    let cb = &components[1];
    let cr = &components[2];
    if cb.h == 0 || cb.v == 0 || cr.h == 0 || cr.v == 0 {
        return None;
    }
    let h_ratio = y.h as f32 / cb.h as f32;
    let v_ratio = y.v as f32 / cb.v as f32;
    let format = if h_ratio == 2.0 && v_ratio == 2.0 {
        "4:2:0"
    } else if h_ratio == 2.0 && v_ratio == 1.0 {
        "4:2:2"
    } else if h_ratio == 1.0 && v_ratio == 1.0 {
        "4:4:4"
    } else {
        return Some(format!("{}:{}:{}", y.h * 2, cb.h * 2, cb.v * 2));
    };
    Some(format.to_string())
}

struct GifMetadata {
    version: String,
    width: u32,
    height: u32,
    global_color_table: Option<usize>,
    color_resolution: u8,
    background_color_index: u8,
    pixel_aspect_ratio: u8,
    frames: usize,
    loop_count: Option<u16>,
    delays: Vec<u16>,
    disposal_methods: Vec<u8>,
    transparency: Vec<bool>,
    comment_count: usize,
    app_extensions: Vec<String>,
}

fn read_gif_metadata(path: &Path) -> Option<GifMetadata> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 13 || !data.starts_with(b"GIF") {
        return None;
    }

    let version = String::from_utf8_lossy(&data[0..6]).to_string();
    let width = u16::from_le_bytes([data[6], data[7]]) as u32;
    let height = u16::from_le_bytes([data[8], data[9]]) as u32;
    let packed = data[10];
    let gct_flag = packed & 0b1000_0000 != 0;
    let color_resolution = ((packed & 0b0111_0000) >> 4) + 1;
    let gct_size = if gct_flag {
        let size = 1 << ((packed & 0b0000_0111) + 1);
        Some(size as usize)
    } else {
        None
    };
    let background_color_index = data[11];
    let pixel_aspect_ratio = data[12];

    let mut pos: usize = 13;
    if let Some(size) = gct_size {
        pos = pos.saturating_add(size * 3);
    }

    let mut frames = 0;
    let mut loop_count = None;
    let mut delays = Vec::new();
    let mut disposal_methods = Vec::new();
    let mut transparency = Vec::new();
    let mut comment_count = 0;
    let mut app_extensions = Vec::new();
    let mut pending_gce: Option<(u16, u8, bool)> = None;

    while pos < data.len() {
        match data[pos] {
            0x2C => {
                // Image descriptor
                frames += 1;
                if let Some((delay, disposal, trans)) = pending_gce.take() {
                    delays.push(delay);
                    disposal_methods.push(disposal);
                    transparency.push(trans);
                } else {
                    delays.push(0);
                    disposal_methods.push(0);
                    transparency.push(false);
                }
                if pos + 9 >= data.len() {
                    break;
                }
                let packed = data[pos + 9];
                let lct_flag = packed & 0b1000_0000 != 0;
                let lct_size = if lct_flag {
                    1 << ((packed & 0b0000_0111) + 1)
                } else {
                    0
                };
                pos += 10;
                if lct_flag {
                    pos = pos.saturating_add(lct_size * 3);
                }
                if pos >= data.len() {
                    break;
                }
                pos += 1; // LZW min code size
                pos = skip_sub_blocks(&data, pos);
            }
            0x21 => {
                if pos + 1 >= data.len() {
                    break;
                }
                let label = data[pos + 1];
                match label {
                    0xF9 => {
                        if pos + 5 < data.len() {
                            let packed = data[pos + 3];
                            let delay = u16::from_le_bytes([data[pos + 4], data[pos + 5]]);
                            let disposal = (packed >> 2) & 0b111;
                            let trans = packed & 0b0000_0001 != 0;
                            pending_gce = Some((delay, disposal, trans));
                        }
                        pos = pos.saturating_add(2);
                        pos = skip_sub_blocks(&data, pos);
                    }
                    0xFF => {
                        if pos + 13 < data.len() {
                            let app = String::from_utf8_lossy(&data[pos + 3..pos + 11])
                                .trim()
                                .to_string();
                            if !app.is_empty() {
                                app_extensions.push(app.clone());
                            }
                            if app.starts_with("NETSCAPE") {
                                let data_start = pos + 14;
                                if data_start + 2 < data.len() && data[data_start] == 0x03 {
                                    loop_count = Some(u16::from_le_bytes([
                                        data[data_start + 1],
                                        data[data_start + 2],
                                    ]));
                                }
                            }
                        }
                        pos = pos.saturating_add(2);
                        pos = skip_sub_blocks(&data, pos);
                    }
                    0xFE => {
                        comment_count += 1;
                        pos = pos.saturating_add(2);
                        pos = skip_sub_blocks(&data, pos);
                    }
                    _ => {
                        pos = pos.saturating_add(2);
                        pos = skip_sub_blocks(&data, pos);
                    }
                }
            }
            0x3B => break,
            _ => break,
        }
    }

    Some(GifMetadata {
        version,
        width,
        height,
        global_color_table: gct_size,
        color_resolution,
        background_color_index,
        pixel_aspect_ratio,
        frames,
        loop_count,
        delays,
        disposal_methods,
        transparency,
        comment_count,
        app_extensions,
    })
}

fn append_gif_entries(
    section: &mut ReportSection,
    _risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    gif: &GifMetadata,
) -> bool {
    let mut has_entries = false;
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("GIF Versión", &gif.version),
    );

    if let Some(size) = gif.global_color_table {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Tabla de colores global", format!("Sí ({size})")),
        );
    } else {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Tabla de colores global", "No"),
        );
    }

    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info(
            "Resolución de color",
            gif.color_resolution.to_string(),
        ),
    );
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info(
            "Índice de color de fondo",
            gif.background_color_index.to_string(),
        ),
    );
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info(
            "Relación de aspecto de píxel",
            gif.pixel_aspect_ratio.to_string(),
        ),
    );

    if gif.frames > 0 {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Frames", gif.frames.to_string()),
        );
    }
    if let Some(loop_count) = gif.loop_count {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Loop count", loop_count.to_string()),
        );
    }

    if !gif.delays.is_empty() {
        let delays_ms = gif
            .delays
            .iter()
            .take(10)
            .map(|value| format!("{} ms", value.saturating_mul(10)))
            .collect::<Vec<_>>()
            .join(", ");
        let label = if gif.delays.len() > 10 {
            format!("{delays_ms} (+{} más)", gif.delays.len() - 10)
        } else {
            delays_ms
        };
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Delays por frame", label),
        );
    }

    if !gif.disposal_methods.is_empty() {
        let list = gif
            .disposal_methods
            .iter()
            .take(10)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let label = if gif.disposal_methods.len() > 10 {
            format!("{list} (+{} más)", gif.disposal_methods.len() - 10)
        } else {
            list
        };
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Disposal por frame", label),
        );
    }

    if !gif.transparency.is_empty() {
        let has_transparency = gif.transparency.iter().any(|value| *value);
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Transparencia por frame",
                if has_transparency { "Sí" } else { "No" },
            ),
        );
    }

    if gif.comment_count > 0 {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Comentarios", gif.comment_count.to_string()),
        );
    }

    if !gif.app_extensions.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Application extensions",
                format_list_with_limit(&gif.app_extensions, 10),
            ),
        );
    }

    has_entries
}

fn skip_sub_blocks(data: &[u8], mut pos: usize) -> usize {
    while pos < data.len() {
        let size = data[pos] as usize;
        pos += 1;
        if size == 0 {
            break;
        }
        pos = pos.saturating_add(size);
    }
    pos
}

struct WebpMetadata {
    riff_size: u32,
    chunks: Vec<String>,
    dimensions: Option<(u32, u32)>,
    has_alpha: bool,
    is_animated: bool,
    frame_count: Option<usize>,
    loop_count: Option<u16>,
    duration_ms: Option<u32>,
    compression: Option<&'static str>,
    icc_profile: Option<Vec<u8>>,
    exif_present: bool,
    xmp_packet: Option<String>,
}

fn read_webp_metadata(path: &Path) -> Option<WebpMetadata> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WEBP" {
        return None;
    }
    let riff_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let mut chunks = Vec::new();
    let mut dimensions = None;
    let mut has_alpha = false;
    let mut is_animated = false;
    let mut frame_count = 0_usize;
    let mut loop_count = None;
    let mut duration_ms = 0_u32;
    let mut compression = None;
    let mut icc_profile = None;
    let mut exif_present = false;
    let mut xmp_packet = None;

    loop {
        let mut chunk_header = [0_u8; 8];
        if file.read_exact(&mut chunk_header).is_err() {
            break;
        }
        let chunk_type = String::from_utf8_lossy(&chunk_header[0..4]).to_string();
        let size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as usize;
        chunks.push(chunk_type.clone());

        match chunk_type.as_str() {
            "VP8X" => {
                let mut payload = vec![0_u8; size.min(10)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                if payload.len() >= 10 {
                    let flags = payload[0];
                    has_alpha |= flags & 0b0001_0000 != 0;
                    is_animated |= flags & 0b0000_0010 != 0;
                    let width = 1 + (payload[4] as u32)
                        + ((payload[5] as u32) << 8)
                        + ((payload[6] as u32) << 16);
                    let height = 1 + (payload[7] as u32)
                        + ((payload[8] as u32) << 8)
                        + ((payload[9] as u32) << 16);
                    dimensions = Some((width, height));
                }
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "VP8 " => {
                compression = Some("Lossy");
                let mut payload = vec![0_u8; size.min(10)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                if payload.len() >= 10 && payload[3..6] == [0x9D, 0x01, 0x2A] {
                    let width = u16::from_le_bytes([payload[6], payload[7]]) & 0x3FFF;
                    let height = u16::from_le_bytes([payload[8], payload[9]]) & 0x3FFF;
                    dimensions = Some((width as u32, height as u32));
                }
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "VP8L" => {
                compression = Some("Lossless");
                let mut payload = vec![0_u8; size.min(5)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                if payload.len() >= 5 && payload[0] == 0x2F {
                    let b1 = payload[1] as u32;
                    let b2 = payload[2] as u32;
                    let b3 = payload[3] as u32;
                    let b4 = payload[4] as u32;
                    let width = 1 + ((b1 | ((b2 & 0x3F) << 8)) & 0x3FFF);
                    let height = 1 + (((b2 >> 6) | (b3 << 2) | ((b4 & 0x0F) << 10)) & 0x3FFF);
                    dimensions = Some((width, height));
                }
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "ANIM" => {
                let mut payload = vec![0_u8; size.min(6)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                if payload.len() >= 6 {
                    loop_count = Some(u16::from_le_bytes([payload[4], payload[5]]));
                }
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "ANMF" => {
                frame_count += 1;
                let mut payload = vec![0_u8; size.min(16)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                if payload.len() >= 16 {
                    let duration = (payload[12] as u32)
                        | ((payload[13] as u32) << 8)
                        | ((payload[14] as u32) << 16);
                    duration_ms = duration_ms.saturating_add(duration);
                }
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "EXIF" => {
                exif_present = true;
                if file.seek(SeekFrom::Current(size as i64)).is_err() {
                    break;
                }
            }
            "XMP " => {
                let mut payload = vec![0_u8; size.min(128 * 1024)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                xmp_packet = Some(String::from_utf8_lossy(&payload).to_string());
                if size > payload.len() && file.seek(SeekFrom::Current((size - payload.len()) as i64)).is_err() {
                    break;
                }
            }
            "ICCP" => {
                let mut payload = vec![0_u8; size.min(256 * 1024)];
                if file.read_exact(&mut payload).is_err() {
                    break;
                }
                let payload_len = payload.len();
                icc_profile = Some(payload);
                if size > payload_len
                    && file
                        .seek(SeekFrom::Current((size - payload_len) as i64))
                        .is_err()
                {
                    break;
                }
            }
            _ => {
                if file.seek(SeekFrom::Current(size as i64)).is_err() {
                    break;
                }
            }
        }

        if size % 2 == 1 {
            let _ = file.seek(SeekFrom::Current(1));
        }
    }

    Some(WebpMetadata {
        riff_size,
        chunks,
        dimensions,
        has_alpha,
        is_animated,
        frame_count: if frame_count > 0 { Some(frame_count) } else { None },
        loop_count,
        duration_ms: if duration_ms > 0 { Some(duration_ms) } else { None },
        compression,
        icc_profile,
        exif_present,
        xmp_packet,
    })
}

fn append_webp_entries(
    section: &mut ReportSection,
    _risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    webp: &WebpMetadata,
) -> bool {
    let mut has_entries = false;
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Tamaño RIFF", webp.riff_size.to_string()),
    );
    if !webp.chunks.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Chunks presentes", format_list_with_limit(&webp.chunks, 12)),
        );
    }
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Tiene alpha", if webp.has_alpha { "Sí" } else { "No" }),
    );
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Es animado", if webp.is_animated { "Sí" } else { "No" }),
    );

    if let Some(count) = webp.frame_count {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Frames", count.to_string()),
        );
    }
    if let Some(loop_count) = webp.loop_count {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Loop count", loop_count.to_string()),
        );
    }
    if let Some(duration) = webp.duration_ms {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Duración total", format!("{duration} ms")),
        );
    }

    if let Some(compression) = webp.compression {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Compresión", compression),
        );
    }

    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("EXIF", if webp.exif_present { "Sí" } else { "No" }),
    );

    has_entries
}

struct TiffMetadata {
    endianness: &'static str,
    bigtiff: bool,
    ifds: Vec<TiffIfd>,
    tag_ids: Vec<u16>,
    dimensions: Option<(u32, u32)>,
    icc_profile: Option<Vec<u8>>,
    xmp_packet: Option<String>,
    iptc_present: bool,
}

struct TiffIfd {
    width: Option<u32>,
    height: Option<u32>,
    bits_per_sample: Option<String>,
    samples_per_pixel: Option<u16>,
    photometric: Option<String>,
    compression: Option<String>,
    planar_config: Option<String>,
    orientation: Option<String>,
    x_resolution: Option<String>,
    y_resolution: Option<String>,
    resolution_unit: Option<String>,
    tiles: Option<String>,
    strips: Option<String>,
    color_map: bool,
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

fn read_tiff_metadata(path: &Path) -> Option<TiffMetadata> {
    let mut file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len() as u64;
    let mut header = [0_u8; 8];
    file.read_exact(&mut header).ok()?;
    let endian = match &header[0..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    let magic = read_u16_from_slice(&header[2..4], endian);
    let bigtiff = magic == 43;
    let mut first_ifd = if bigtiff {
        let mut extra = [0_u8; 8];
        file.read_exact(&mut extra).ok()?;
        read_u64_from_slice(&extra, endian)
    } else {
        read_u32_from_slice(&header[4..8], endian) as u64
    };

    let mut ifds = Vec::new();
    let mut tag_ids = Vec::new();
    let mut icc_profile = None;
    let mut xmp_packet = None;
    let mut iptc_present = false;
    let mut ifd_index = 0;
    while first_ifd != 0 && first_ifd < size && ifd_index < 16 {
        if file.seek(SeekFrom::Start(first_ifd)).is_err() {
            break;
        }
        let entries = if bigtiff {
            read_u64_from_reader(&mut file, endian)? as usize
        } else {
            read_u16_from_reader(&mut file, endian)? as usize
        };
        let mut ifd = TiffIfd {
            width: None,
            height: None,
            bits_per_sample: None,
            samples_per_pixel: None,
            photometric: None,
            compression: None,
            planar_config: None,
            orientation: None,
            x_resolution: None,
            y_resolution: None,
            resolution_unit: None,
            tiles: None,
            strips: None,
            color_map: false,
        };
        let inline_size = if bigtiff { 8 } else { 4 };
        for _ in 0..entries {
            let tag = read_u16_from_reader(&mut file, endian)?;
            let field_type = read_u16_from_reader(&mut file, endian)?;
            let count = if bigtiff {
                read_u64_from_reader(&mut file, endian)?
            } else {
                read_u32_from_reader(&mut file, endian)? as u64
            };
            let value_offset = if bigtiff {
                read_u64_from_reader(&mut file, endian)?
            } else {
                read_u32_from_reader(&mut file, endian)? as u64
            };
            tag_ids.push(tag);

            let total_size = tiff_type_size(field_type).saturating_mul(count as usize);
            let value = read_tiff_value(
                &mut file,
                endian,
                value_offset,
                total_size,
                inline_size,
                size,
            );

            match tag {
                256 => ifd.width = tiff_first_u32(&value, endian),
                257 => ifd.height = tiff_first_u32(&value, endian),
                258 => ifd.bits_per_sample = tiff_u16_list(&value, endian),
                259 => ifd.compression = tiff_compression_label(tiff_first_u32(&value, endian)),
                262 => ifd.photometric = tiff_photometric_label(tiff_first_u32(&value, endian)),
                273 => ifd.strips = tiff_count_label(count, "strips"),
                274 => ifd.orientation = tiff_orientation_label(tiff_first_u32(&value, endian)),
                277 => ifd.samples_per_pixel = tiff_first_u16(&value, endian),
                279 => ifd.strips = tiff_count_label(count, "strips"),
                282 => ifd.x_resolution = tiff_rational(&value, endian),
                283 => ifd.y_resolution = tiff_rational(&value, endian),
                284 => ifd.planar_config = tiff_planar_label(tiff_first_u32(&value, endian)),
                296 => ifd.resolution_unit = tiff_resolution_unit_label(tiff_first_u32(&value, endian)),
                322 => ifd.tiles = tiff_count_label(count, "tiles"),
                323 => ifd.tiles = tiff_count_label(count, "tiles"),
                324 => ifd.tiles = tiff_count_label(count, "tiles"),
                325 => ifd.tiles = tiff_count_label(count, "tiles"),
                320 => ifd.color_map = true,
                33723 => iptc_present = true,
                34675 => {
                    if icc_profile.is_none() {
                        icc_profile = value;
                    }
                }
                700 => {
                    if xmp_packet.is_none() {
                        if let Some(value) = value {
                            let text = String::from_utf8_lossy(&value).to_string();
                            if !text.trim().is_empty() {
                                xmp_packet = Some(text);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let next_ifd = if bigtiff {
            read_u64_from_reader(&mut file, endian).unwrap_or(0)
        } else {
            read_u32_from_reader(&mut file, endian).unwrap_or(0) as u64
        };
        ifds.push(ifd);
        first_ifd = next_ifd;
        ifd_index += 1;
    }

    let dimensions = ifds
        .first()
        .and_then(|ifd| Some((ifd.width?, ifd.height?)));

    Some(TiffMetadata {
        endianness: match endian {
            Endian::Little => "Little-endian (II)",
            Endian::Big => "Big-endian (MM)",
        },
        bigtiff,
        ifds,
        tag_ids,
        dimensions,
        icc_profile,
        xmp_packet,
        iptc_present,
    })
}

fn append_tiff_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    tiff: &TiffMetadata,
) -> bool {
    let mut has_entries = false;
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Endianness", tiff.endianness),
    );
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("BigTIFF", if tiff.bigtiff { "Sí" } else { "No" }),
    );
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Número de IFDs", tiff.ifds.len().to_string()),
    );
    if !tiff.tag_ids.is_empty() {
        let mut tags = tiff
            .tag_ids
            .iter()
            .map(|tag| format!("{tag:#06x}"))
            .collect::<Vec<_>>();
        tags.sort();
        tags.dedup();
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Tags TIFF", format_list_with_limit(&tags, 12)),
        );
    }

    for (index, ifd) in tiff.ifds.iter().enumerate() {
        let prefix = format!("IFD {} · ", index + 1);
        if let Some(width) = ifd.width {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Ancho"), width.to_string()),
            );
        }
        if let Some(height) = ifd.height {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Alto"), height.to_string()),
            );
        }
        if let Some(bits) = &ifd.bits_per_sample {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Bits por muestra"), bits),
            );
        }
        if let Some(samples) = ifd.samples_per_pixel {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Samples por pixel"), samples.to_string()),
            );
        }
        if let Some(value) = &ifd.photometric {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Interpretación"), value),
            );
        }
        if let Some(value) = &ifd.compression {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Compresión"), value),
            );
        }
        if let Some(value) = &ifd.planar_config {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Planar"), value),
            );
        }
        if let Some(value) = &ifd.orientation {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Orientación"), value),
            );
        }
        if let Some(value) = &ifd.resolution_unit {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Unidad resolución"), value),
            );
        }
        if let Some(value) = &ifd.x_resolution {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Resolución X"), value),
            );
        }
        if let Some(value) = &ifd.y_resolution {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Resolución Y"), value),
            );
        }
        if let Some(value) = &ifd.tiles {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Tiles"), value),
            );
        }
        if let Some(value) = &ifd.strips {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Strips"), value),
            );
        }
        if ifd.color_map {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info(format!("{prefix}Color map"), "Sí"),
            );
        }
    }

    if tiff.iptc_present {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::warning("IPTC embebido", "Detectado"),
        );
        risks.push(ReportEntry::warning("IPTC embebido", "Detectado"));
    }

    has_entries
}

fn tiff_type_size(field_type: u16) -> usize {
    match field_type {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 => 8,
        16 | 17 => 8,
        _ => 0,
    }
}

fn read_u16_from_slice(slice: &[u8], endian: Endian) -> u16 {
    match endian {
        Endian::Little => u16::from_le_bytes([slice[0], slice[1]]),
        Endian::Big => u16::from_be_bytes([slice[0], slice[1]]),
    }
}

fn read_u32_from_slice(slice: &[u8], endian: Endian) -> u32 {
    match endian {
        Endian::Little => u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]),
        Endian::Big => u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]),
    }
}

fn read_u64_from_slice(slice: &[u8], endian: Endian) -> u64 {
    match endian {
        Endian::Little => u64::from_le_bytes([
            slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
        ]),
        Endian::Big => u64::from_be_bytes([
            slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
        ]),
    }
}

fn read_u16_from_reader<R: Read>(reader: &mut R, endian: Endian) -> Option<u16> {
    let mut buffer = [0_u8; 2];
    reader.read_exact(&mut buffer).ok()?;
    Some(read_u16_from_slice(&buffer, endian))
}

fn read_u32_from_reader<R: Read>(reader: &mut R, endian: Endian) -> Option<u32> {
    let mut buffer = [0_u8; 4];
    reader.read_exact(&mut buffer).ok()?;
    Some(read_u32_from_slice(&buffer, endian))
}

fn read_u64_from_reader<R: Read>(reader: &mut R, endian: Endian) -> Option<u64> {
    let mut buffer = [0_u8; 8];
    reader.read_exact(&mut buffer).ok()?;
    Some(read_u64_from_slice(&buffer, endian))
}

fn read_tiff_value(
    reader: &mut File,
    endian: Endian,
    offset: u64,
    size: usize,
    inline_size: usize,
    file_size: u64,
) -> Option<Vec<u8>> {
    if size == 0 {
        return None;
    }
    if size <= inline_size {
        let mut bytes = if inline_size == 4 {
            match endian {
                Endian::Little => (offset as u32).to_le_bytes().to_vec(),
                Endian::Big => (offset as u32).to_be_bytes().to_vec(),
            }
        } else {
            match endian {
                Endian::Little => offset.to_le_bytes().to_vec(),
                Endian::Big => offset.to_be_bytes().to_vec(),
            }
        };
        bytes.truncate(size);
        return Some(bytes);
    }
    if offset >= file_size {
        return None;
    }
    if reader.seek(SeekFrom::Start(offset)).is_err() {
        return None;
    }
    let mut buffer = vec![0_u8; size];
    reader.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn tiff_first_u16(value: &Option<Vec<u8>>, endian: Endian) -> Option<u16> {
    let bytes = value.as_ref()?;
    if bytes.len() < 2 {
        return None;
    }
    Some(read_u16_from_slice(&bytes[0..2], endian))
}

fn tiff_first_u32(value: &Option<Vec<u8>>, endian: Endian) -> Option<u32> {
    let bytes = value.as_ref()?;
    if bytes.len() < 4 {
        return None;
    }
    Some(read_u32_from_slice(&bytes[0..4], endian))
}

fn tiff_u16_list(value: &Option<Vec<u8>>, endian: Endian) -> Option<String> {
    let bytes = value.as_ref()?;
    if bytes.len() < 2 {
        return None;
    }
    let mut values = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        values.push(read_u16_from_slice(chunk, endian).to_string());
    }
    if values.is_empty() {
        None
    } else {
        Some(values.join(", "))
    }
}

fn tiff_rational(value: &Option<Vec<u8>>, endian: Endian) -> Option<String> {
    let bytes = value.as_ref()?;
    if bytes.len() < 8 {
        return None;
    }
    let num = read_u32_from_slice(&bytes[0..4], endian) as f64;
    let den = read_u32_from_slice(&bytes[4..8], endian) as f64;
    if den == 0.0 {
        return None;
    }
    Some(format!("{:.4}", num / den))
}

fn tiff_count_label(count: u64, label: &str) -> Option<String> {
    if count == 0 {
        None
    } else {
        Some(format!("{count} {label}"))
    }
}

fn tiff_compression_label(value: Option<u32>) -> Option<String> {
    let label = match value? {
        1 => "None",
        5 => "LZW",
        6 => "JPEG (deprecated)",
        7 => "JPEG",
        8 => "Deflate",
        32773 => "PackBits",
        _ => "Otro",
    };
    Some(label.to_string())
}

fn tiff_photometric_label(value: Option<u32>) -> Option<String> {
    let label = match value? {
        0 => "WhiteIsZero",
        1 => "BlackIsZero",
        2 => "RGB",
        3 => "Palette",
        4 => "Transparency Mask",
        5 => "CMYK",
        6 => "YCbCr",
        8 => "CIELab",
        _ => "Otro",
    };
    Some(label.to_string())
}

fn tiff_planar_label(value: Option<u32>) -> Option<String> {
    let label = match value? {
        1 => "Chunky",
        2 => "Planar",
        _ => "Otro",
    };
    Some(label.to_string())
}

fn tiff_resolution_unit_label(value: Option<u32>) -> Option<String> {
    let label = match value? {
        1 => "Sin unidad",
        2 => "Inches",
        3 => "Centímetros",
        _ => "Otro",
    };
    Some(label.to_string())
}

fn tiff_orientation_label(value: Option<u32>) -> Option<String> {
    let label = match value? {
        1 => "Arriba-izquierda",
        2 => "Arriba-derecha",
        3 => "Abajo-derecha",
        4 => "Abajo-izquierda",
        5 => "Izquierda-arriba",
        6 => "Derecha-arriba",
        7 => "Derecha-abajo",
        8 => "Izquierda-abajo",
        _ => "Otro",
    };
    Some(label.to_string())
}

struct HeifMetadata {
    major_brand: Option<String>,
    compatible_brands: Vec<String>,
    item_count: Option<u32>,
    primary_item_id: Option<u32>,
    box_list: Vec<String>,
    dimensions: Option<(u32, u32)>,
    bit_depth: Option<u8>,
    rotation: Option<String>,
    mirror: Option<String>,
    thumbnails: Option<usize>,
    aux_images: Option<usize>,
    grid: bool,
    icc_profile: Option<Vec<u8>>,
    nclx: Option<String>,
    xmp_packet: Option<String>,
}

fn read_heif_metadata(path: &Path) -> Option<HeifMetadata> {
    let mut file = File::open(path).ok()?;
    let mut major_brand = None;
    let mut compatible_brands = Vec::new();
    let mut meta_payload = None;

    loop {
        let Some(header) = read_box_header(&mut file) else {
            break;
        };
        let box_type = String::from_utf8_lossy(&header.kind).to_string();
        match box_type.as_str() {
            "ftyp" => {
                let payload = read_box_payload(&mut file, &header, 1024 * 1024)?;
                if payload.len() >= 8 {
                    major_brand = Some(String::from_utf8_lossy(&payload[0..4]).to_string());
                    let mut offset = 8;
                    while offset + 4 <= payload.len() {
                        let brand = String::from_utf8_lossy(&payload[offset..offset + 4]).to_string();
                        compatible_brands.push(brand);
                        offset += 4;
                    }
                }
            }
            "meta" => {
                meta_payload = read_box_payload(&mut file, &header, 8 * 1024 * 1024);
            }
            _ => {
                let _ = file.seek(SeekFrom::Current(header.payload_size as i64));
            }
        }
    }

    let mut meta = HeifMetadata {
        major_brand,
        compatible_brands,
        item_count: None,
        primary_item_id: None,
        box_list: Vec::new(),
        dimensions: None,
        bit_depth: None,
        rotation: None,
        mirror: None,
        thumbnails: None,
        aux_images: None,
        grid: false,
        icc_profile: None,
        nclx: None,
        xmp_packet: None,
    };

    if let Some(payload) = meta_payload {
        parse_heif_meta(&payload, &mut meta);
        if meta.xmp_packet.is_none() {
            meta.xmp_packet = extract_xmp_packet_from_bytes(&payload);
        }
    }

    if meta.major_brand.is_none()
        && meta.compatible_brands.is_empty()
        && meta.item_count.is_none()
        && meta.primary_item_id.is_none()
        && meta.dimensions.is_none()
    {
        return None;
    }

    Some(meta)
}

fn append_heif_entries(
    section: &mut ReportSection,
    _risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    heif: &HeifMetadata,
) -> bool {
    let mut has_entries = false;
    if let Some(brand) = &heif.major_brand {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Major brand", brand),
        );
    }
    if !heif.compatible_brands.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Compatible brands",
                format_list_with_limit(&heif.compatible_brands, 10),
            ),
        );
    }
    if let Some(count) = heif.item_count {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Items", count.to_string()),
        );
    }
    if let Some(primary) = heif.primary_item_id {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Primary item id", primary.to_string()),
        );
    }
    if !heif.box_list.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Cajas metadata",
                format_list_with_limit(&heif.box_list, 12),
            ),
        );
    }
    if let Some((width, height)) = heif.dimensions {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Ancho", width.to_string()),
        );
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Alto", height.to_string()),
        );
    }
    if let Some(bits) = heif.bit_depth {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Profundidad de bits", bits.to_string()),
        );
    }
    if let Some(value) = &heif.nclx {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Perfil de color", value),
        );
    }
    if let Some(value) = &heif.rotation {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Rotación", value),
        );
    }
    if let Some(value) = &heif.mirror {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Espejo", value),
        );
    }
    if let Some(count) = heif.thumbnails {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Miniaturas", count.to_string()),
        );
    }
    if let Some(count) = heif.aux_images {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Auxiliares", count.to_string()),
        );
    }
    if heif.grid {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Grid", "Sí"));
    }
    has_entries
}

struct BoxHeader {
    kind: [u8; 4],
    payload_size: u64,
}

fn read_box_header<R: Read>(reader: &mut R) -> Option<BoxHeader> {
    let mut buffer = [0_u8; 8];
    reader.read_exact(&mut buffer).ok()?;
    let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as u64;
    let mut kind = [0_u8; 4];
    kind.copy_from_slice(&buffer[4..8]);
    if size == 1 {
        let mut ext = [0_u8; 8];
        reader.read_exact(&mut ext).ok()?;
        let full_size = u64::from_be_bytes(ext);
        let payload_size = full_size.saturating_sub(16);
        return Some(BoxHeader {
            kind,
            payload_size,
        });
    }
    let payload_size = size.saturating_sub(8);
    Some(BoxHeader {
        kind,
        payload_size,
    })
}

fn read_box_payload<R: Read>(reader: &mut R, header: &BoxHeader, limit: usize) -> Option<Vec<u8>> {
    let size = header.payload_size as usize;
    if size > limit {
        let mut skip = vec![0_u8; limit];
        let _ = reader.read_exact(&mut skip);
        let remaining = size.saturating_sub(limit);
        let _ = reader.by_ref().take(remaining as u64).read_to_end(&mut Vec::new());
        return Some(skip);
    }
    let mut buffer = vec![0_u8; size];
    reader.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn parse_heif_meta(payload: &[u8], meta: &mut HeifMetadata) {
    if payload.len() < 4 {
        return;
    }
    let mut cursor = Cursor::new(payload);
    let mut header = [0_u8; 4];
    let _ = cursor.read_exact(&mut header); // version + flags
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        meta.box_list.push(name.clone());
        let data = match read_box_payload(&mut cursor, &header, 4 * 1024 * 1024) {
            Some(value) => value,
            None => break,
        };
        match name.as_str() {
            "pitm" => {
                if data.len() >= 6 {
                    let version = data[0];
                    let id = if version == 0 && data.len() >= 6 {
                        u16::from_be_bytes([data[4], data[5]]) as u32
                    } else if data.len() >= 8 {
                        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
                    } else {
                        0
                    };
                    if id != 0 {
                        meta.primary_item_id = Some(id);
                    }
                }
            }
            "iinf" => {
                if data.len() >= 8 {
                    let version = data[0];
                    let count = if version == 0 {
                        u16::from_be_bytes([data[4], data[5]]) as u32
                    } else {
                        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
                    };
                    meta.item_count = Some(count);
                    meta.thumbnails = Some(data.windows(4).filter(|w| *w == b"thmb").count());
                    meta.aux_images = Some(data.windows(4).filter(|w| *w == b"auxl").count());
                    if data.windows(4).any(|w| w == b"grid") {
                        meta.grid = true;
                    }
                }
            }
            "iprp" => parse_heif_iprp(&data, meta),
            _ => {}
        }
    }
}

fn parse_heif_iprp(payload: &[u8], meta: &mut HeifMetadata) {
    let mut cursor = Cursor::new(payload);
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let data = match read_box_payload(&mut cursor, &header, 2 * 1024 * 1024) {
            Some(value) => value,
            None => break,
        };
        if name == "ipco" {
            parse_heif_ipco(&data, meta);
        }
    }
}

fn parse_heif_ipco(payload: &[u8], meta: &mut HeifMetadata) {
    let mut cursor = Cursor::new(payload);
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let data = match read_box_payload(&mut cursor, &header, 2 * 1024 * 1024) {
            Some(value) => value,
            None => break,
        };
        match name.as_str() {
            "ispe" => {
                if data.len() >= 12 {
                    let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                    let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
                    meta.dimensions = Some((width, height));
                }
            }
            "pixi" => {
                if data.len() >= 6 {
                    let count = data[4] as usize;
                    if data.len() >= 5 + count {
                        meta.bit_depth = Some(data[5]);
                    }
                }
            }
            "irot" => {
                if data.len() >= 5 {
                    let value = data[4] & 0x03;
                    meta.rotation = Some(format!("{}°", value as u16 * 90));
                }
            }
            "imir" => {
                if data.len() >= 5 {
                    let value = data[4] & 0x01;
                    meta.mirror = Some(if value == 1 { "Sí" } else { "No" }.to_string());
                }
            }
            "colr" => {
                if data.len() >= 8 {
                    let color_type = &data[4..8];
                    match color_type {
                        b"nclx" if data.len() >= 15 => {
                            let primaries = u16::from_be_bytes([data[8], data[9]]);
                            let transfer = u16::from_be_bytes([data[10], data[11]]);
                            let matrix = u16::from_be_bytes([data[12], data[13]]);
                            let full = data[14] & 0x80 != 0;
                            meta.nclx = Some(format!(
                                "nclx (prim:{primaries}, trans:{transfer}, matrix:{matrix}, full:{full})"
                            ));
                        }
                        b"rICC" | b"prof" => {
                            meta.icc_profile = Some(data[8..].to_vec());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

struct SvgMetadata {
    xml_version: Option<String>,
    encoding: Option<String>,
    doctype: Option<String>,
    namespaces: Vec<String>,
    parse_error: bool,
    width: Option<String>,
    height: Option<String>,
    view_box: Option<String>,
    preserve_aspect_ratio: Option<String>,
    units: Option<String>,
    title: Option<String>,
    desc: Option<String>,
    metadata: Option<String>,
    xmp_packet: Option<String>,
    scripts: usize,
    external_links: Vec<String>,
    data_images: usize,
    remote_refs: Vec<String>,
    font_families: Vec<String>,
    dimensions: Option<(u32, u32)>,
}

fn read_svg_metadata(path: &Path) -> Option<SvgMetadata> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let (xml_version, encoding) = parse_xml_declaration(&text);
    let doctype = parse_doctype(&text);

    let mut meta = SvgMetadata {
        xml_version,
        encoding,
        doctype,
        namespaces: Vec::new(),
        parse_error: false,
        width: None,
        height: None,
        view_box: None,
        preserve_aspect_ratio: None,
        units: None,
        title: None,
        desc: None,
        metadata: None,
        xmp_packet: None,
        scripts: 0,
        external_links: Vec::new(),
        data_images: 0,
        remote_refs: Vec::new(),
        font_families: Vec::new(),
        dimensions: None,
    };

    let root = match Element::parse(text.as_bytes()) {
        Ok(root) => root,
        Err(_) => {
            meta.parse_error = true;
            return Some(meta);
        }
    };

    if root.name == "svg" {
        for (key, value) in &root.attributes {
            if key.starts_with("xmlns") {
                meta.namespaces.push(format!("{key}={value}"));
            }
        }
        meta.width = root.attributes.get("width").cloned();
        meta.height = root.attributes.get("height").cloned();
        meta.view_box = root.attributes.get("viewBox").cloned();
        meta.preserve_aspect_ratio = root.attributes.get("preserveAspectRatio").cloned();

        if let (Some(width), Some(height)) = (meta.width.clone(), meta.height.clone()) {
            let (w, w_unit) = parse_svg_length(&width);
            let (h, h_unit) = parse_svg_length(&height);
            if let (Some(w), Some(h)) = (w, h) {
                if w_unit.as_deref().unwrap_or("px") == h_unit.as_deref().unwrap_or("px") {
                    meta.units = w_unit.or(h_unit);
                    meta.dimensions = Some((w.round() as u32, h.round() as u32));
                }
            }
        }
    }

    walk_svg_tree(&root, &mut meta);

    if meta.xmp_packet.is_none() {
        meta.xmp_packet = extract_xmp_packet_from_bytes(text.as_bytes());
    }

    Some(meta)
}

fn append_svg_entries(
    section: &mut ReportSection,
    _risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    svg: &SvgMetadata,
) -> bool {
    let mut has_entries = false;
    if let Some(version) = &svg.xml_version {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("XML Versión", version),
        );
    }
    if let Some(encoding) = &svg.encoding {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("XML Encoding", encoding),
        );
    }
    if let Some(doctype) = &svg.doctype {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("DOCTYPE", doctype));
    }
    if !svg.namespaces.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Namespaces", format_list_with_limit(&svg.namespaces, 8)),
        );
    }
    has_entries |= push_entry_unique(
        section,
        seen,
        ReportEntry::info("Errores XML", if svg.parse_error { "Sí" } else { "No" }),
    );
    if let Some(width) = &svg.width {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Width", width));
    }
    if let Some(height) = &svg.height {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Height", height));
    }
    if let Some(view_box) = &svg.view_box {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("ViewBox", view_box));
    }
    if let Some(value) = &svg.preserve_aspect_ratio {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("PreserveAspectRatio", value),
        );
    }
    if let Some(units) = &svg.units {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Unidades", units));
    }
    if let Some(title) = &svg.title {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Título", title));
    }
    if let Some(desc) = &svg.desc {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Descripción", desc));
    }
    if let Some(meta) = &svg.metadata {
        has_entries |= push_entry_unique(section, seen, ReportEntry::info("Metadata", meta));
    }
    if svg.scripts > 0 {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Scripts embebidos", svg.scripts.to_string()),
        );
    }
    if !svg.external_links.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Enlaces externos",
                format_list_with_limit(&svg.external_links, 10),
            ),
        );
    }
    if svg.data_images > 0 {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("Imágenes embebidas", svg.data_images.to_string()),
        );
    }
    if !svg.remote_refs.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Recursos remotos",
                format_list_with_limit(&svg.remote_refs, 10),
            ),
        );
    }
    if !svg.font_families.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "Fuentes referenciadas",
                format_list_with_limit(&svg.font_families, 10),
            ),
        );
    }
    has_entries
}

fn parse_xml_declaration(text: &str) -> (Option<String>, Option<String>) {
    let Some(start) = text.find("<?xml") else {
        return (None, None);
    };
    let Some(end_rel) = text[start..].find("?>") else {
        return (None, None);
    };
    let decl = &text[start..start + end_rel];
    let version = extract_xml_attr(decl, "version");
    let encoding = extract_xml_attr(decl, "encoding");
    (version, encoding)
}

fn parse_doctype(text: &str) -> Option<String> {
    let start = text.find("<!DOCTYPE")?;
    let end = text[start..].find('>')?;
    Some(text[start..start + end + 1].trim().to_string())
}

fn extract_xml_attr(text: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=\"");
    let start = text.find(&pattern)? + pattern.len();
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

fn parse_svg_length(value: &str) -> (Option<f32>, Option<String>) {
    let mut number = String::new();
    let mut unit = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
        } else {
            unit.push(ch);
        }
    }
    let num = number.parse::<f32>().ok();
    let unit = if unit.trim().is_empty() {
        None
    } else {
        Some(unit.trim().to_string())
    };
    (num, unit)
}

fn walk_svg_tree(element: &Element, meta: &mut SvgMetadata) {
    match element.name.as_str() {
        "title" => {
            if meta.title.is_none() {
                meta.title = Some(element_text_content(element));
            }
        }
        "desc" => {
            if meta.desc.is_none() {
                meta.desc = Some(element_text_content(element));
            }
        }
        "metadata" => {
            let text = element_text_content(element);
            if !text.is_empty() {
                meta.metadata = Some(text.clone());
                if text.contains("<rdf:RDF") || text.contains("<x:xmpmeta") {
                    meta.xmp_packet = Some(text);
                }
            }
        }
        "script" => meta.scripts += 1,
        "image" => {
            if let Some(href) = svg_href(element) {
                if href.starts_with("data:") {
                    meta.data_images += 1;
                }
            }
        }
        _ => {}
    }

    for (key, value) in &element.attributes {
        if key.ends_with("href") {
            if value.starts_with("http://") || value.starts_with("https://") {
                meta.external_links.push(value.to_string());
            }
        }
        if key == "style" {
            extract_font_families(value, &mut meta.font_families);
            extract_remote_refs(value, &mut meta.remote_refs);
        }
        if key == "font-family" {
            extract_font_families(value, &mut meta.font_families);
        }
    }

    for node in &element.children {
        if let XMLNode::Element(child) = node {
            walk_svg_tree(child, meta);
        }
    }
}

fn svg_href(element: &Element) -> Option<&str> {
    element
        .attributes
        .get("href")
        .map(String::as_str)
        .or_else(|| element.attributes.get("xlink:href").map(String::as_str))
}

fn extract_font_families(style: &str, fonts: &mut Vec<String>) {
    for part in style.split(';') {
        if let Some(value) = part.split_once(':') {
            if value.0.trim().eq_ignore_ascii_case("font-family") {
                fonts.extend(
                    value
                        .1
                        .split(',')
                        .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                        .filter(|s| !s.is_empty()),
                );
            }
        }
    }
}

fn extract_remote_refs(style: &str, refs: &mut Vec<String>) {
    if style.contains("url(") {
        refs.push(style.to_string());
    }
}

fn element_text_content(element: &Element) -> String {
    let mut content = String::new();
    for node in &element.children {
        if let XMLNode::Text(text) = node {
            content.push_str(text);
        }
    }
    content.trim().to_string()
}

fn decode_ztxt(chunk: &ZTXtChunk) -> Option<String> {
    let mut clone = chunk.clone();
    clone.decompress_text_with_limit(TEXT_DECOMPRESS_LIMIT).ok()?;
    clone.get_text().ok()
}

fn decode_itxt(chunk: &ITXtChunk) -> Option<String> {
    let mut clone = chunk.clone();
    if clone.compressed {
        clone.decompress_text_with_limit(TEXT_DECOMPRESS_LIMIT).ok()?;
    }
    clone.get_text().ok()
}

fn is_xmp_keyword(keyword: &str) -> bool {
    let lowered = keyword.to_lowercase();
    matches!(
        lowered.as_str(),
        "xml:com.adobe.xmp" | "xml:com.adobe.xmpmeta" | "xmp" | "xmpmeta"
    ) || lowered.contains("xmp")
}

fn scan_xmp_packet(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(SIDECAR_SCAN_LIMIT).read_to_end(&mut buffer).ok()?;
    extract_xmp_packet_from_bytes(&buffer)
}

fn extract_xmp_packet_from_bytes(buffer: &[u8]) -> Option<String> {
    let (start_tag, end_tag): (&[u8], &[u8]) =
        if find_subslice(buffer, b"<x:xmpmeta").is_some() {
            (b"<x:xmpmeta", b"</x:xmpmeta>")
        } else if find_subslice(buffer, b"<rdf:RDF").is_some() {
            (b"<rdf:RDF", b"</rdf:RDF>")
    } else {
        return None;
    };

    let start = find_subslice(buffer, start_tag)?;
    let end = find_subslice(&buffer[start..], end_tag)?;
    let end_index = start + end + end_tag.len();
    let slice = &buffer[start..end_index];
    Some(String::from_utf8_lossy(slice).to_string())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn detect_iptc(path: &Path) -> bool {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut buffer = Vec::new();
    if file
        .take(SIDECAR_SCAN_LIMIT)
        .read_to_end(&mut buffer)
        .is_err()
    {
        return false;
    }

    contains_bytes(&buffer, b"Photoshop 3.0")
        && contains_bytes(&buffer, b"8BIM")
        && contains_bytes(&buffer, b"IPTC")
}

#[derive(Default)]
struct IptcMetadata {
    headline: Option<String>,
    caption: Option<String>,
    keywords: Vec<String>,
    author: Option<String>,
    credit: Option<String>,
    source: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<String>,
    date: Option<String>,
    time: Option<String>,
}

fn extract_iptc_metadata(path: &Path) -> Option<IptcMetadata> {
    let file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(SIDECAR_SCAN_LIMIT).read_to_end(&mut buffer).ok()?;
    let mut offset = 0;
    let mut meta = IptcMetadata::default();
    while let Some(pos) = find_subslice(&buffer[offset..], b"8BIM") {
        let start = offset + pos;
        if start + 8 >= buffer.len() {
            break;
        }
        let resource_id =
            u16::from_be_bytes([buffer[start + 4], buffer[start + 5]]);
        let name_len = buffer[start + 6] as usize;
        let mut name_end = start + 7 + name_len;
        if name_end % 2 == 1 {
            name_end += 1;
        }
        if name_end + 4 > buffer.len() {
            break;
        }
        let size = u32::from_be_bytes([
            buffer[name_end],
            buffer[name_end + 1],
            buffer[name_end + 2],
            buffer[name_end + 3],
        ]) as usize;
        let data_start = name_end + 4;
        if data_start + size > buffer.len() {
            break;
        }
        if resource_id == 0x0404 {
            parse_iptc_dataset(&buffer[data_start..data_start + size], &mut meta);
        }
        offset = data_start + size;
    }

    if meta.headline.is_some()
        || meta.caption.is_some()
        || !meta.keywords.is_empty()
        || meta.author.is_some()
        || meta.credit.is_some()
        || meta.source.is_some()
        || meta.city.is_some()
        || meta.state.is_some()
        || meta.country.is_some()
        || meta.date.is_some()
    {
        Some(meta)
    } else {
        None
    }
}

fn parse_iptc_dataset(data: &[u8], meta: &mut IptcMetadata) {
    let mut i = 0;
    while i + 5 <= data.len() {
        if data[i] != 0x1C {
            i += 1;
            continue;
        }
        let record = data[i + 1];
        let dataset = data[i + 2];
        let length = u16::from_be_bytes([data[i + 3], data[i + 4]]) as usize;
        let start = i + 5;
        let end = start.saturating_add(length);
        if end > data.len() {
            break;
        }
        if record == 2 {
            let value = String::from_utf8_lossy(&data[start..end]).trim().to_string();
            if value.is_empty() {
                i = end;
                continue;
            }
            match dataset {
                25 => meta.keywords.push(value),
                55 => meta.date = Some(value),
                60 => meta.time = Some(value),
                80 => meta.author = Some(value),
                90 => meta.city = Some(value),
                95 => meta.state = Some(value),
                101 => meta.country = Some(value),
                105 => meta.headline = Some(value),
                110 => meta.credit = Some(value),
                115 => meta.source = Some(value),
                120 => meta.caption = Some(value),
                _ => {}
            }
        }
        i = end;
    }
}

fn append_iptc_entries(
    section: &mut ReportSection,
    risks: &mut Vec<ReportEntry>,
    seen: &mut HashSet<String>,
    iptc: &IptcMetadata,
) -> bool {
    let mut has_entries = false;
    if let Some(value) = &iptc.headline {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("IPTC Título", value),
        );
    }
    if let Some(value) = &iptc.caption {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("IPTC Descripción", value),
        );
    }
    if !iptc.keywords.is_empty() {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info(
                "IPTC Keywords",
                format_list_with_limit(&iptc.keywords, 10),
            ),
        );
    }
    if let Some(value) = &iptc.author {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::warning("IPTC Autor", value),
        );
        risks.push(ReportEntry::warning("IPTC Autor", value.to_string()));
    }
    if let Some(value) = &iptc.credit {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::warning("IPTC Crédito", value),
        );
        risks.push(ReportEntry::warning("IPTC Crédito", value.to_string()));
    }
    if let Some(value) = &iptc.source {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::warning("IPTC Fuente", value),
        );
        risks.push(ReportEntry::warning("IPTC Fuente", value.to_string()));
    }
    if let Some(value) = &iptc.city {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("IPTC Ciudad", value),
        );
    }
    if let Some(value) = &iptc.state {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("IPTC Estado", value),
        );
    }
    if let Some(value) = &iptc.country {
        has_entries |= push_entry_unique(
            section,
            seen,
            ReportEntry::info("IPTC País", value),
        );
    }
    if iptc.date.is_some() || iptc.time.is_some() {
        let value = match (&iptc.date, &iptc.time) {
            (Some(date), Some(time)) => format!("{date} {time}"),
            (Some(date), None) => date.to_string(),
            (None, Some(time)) => time.to_string(),
            _ => "".to_string(),
        };
        if !value.is_empty() {
            has_entries |= push_entry_unique(
                section,
                seen,
                ReportEntry::info("IPTC Fecha de creación", value),
            );
        }
    }
    has_entries
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn format_list_with_limit(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            unique.push(item.clone());
        }
    }
    let displayed = unique.iter().take(limit).cloned().collect::<Vec<_>>().join(", ");
    if unique.len() > limit {
        format!("{displayed} (+{} más)", unique.len() - limit)
    } else {
        displayed
    }
}

fn push_entry_unique(
    section: &mut ReportSection,
    seen: &mut HashSet<String>,
    entry: ReportEntry,
) -> bool {
    if !seen.insert(entry.label.clone()) {
        return false;
    }
    section.entries.push(entry);
    true
}

struct ExifSpec {
    tag: Tag,
    label: &'static str,
    level: EntryLevel,
}

impl ExifSpec {
    fn info(tag: Tag, label: &'static str) -> Self {
        Self {
            tag,
            label,
            level: EntryLevel::Info,
        }
    }

    fn warning(tag: Tag, label: &'static str) -> Self {
        Self {
            tag,
            label,
            level: EntryLevel::Warning,
        }
    }
}

struct PngMetadata {
    width: u32,
    height: u32,
    bit_depth: png::BitDepth,
    color_type: png::ColorType,
    interlaced: bool,
    gamma: Option<f32>,
    srgb_intent: Option<png::SrgbRenderingIntent>,
    icc_profile: Option<Vec<u8>>,
    icc_name: Option<String>,
    chromaticities: Option<String>,
    phys: Option<PngPhys>,
    chunk_list: Vec<String>,
    chunk_counts: HashMap<String, usize>,
    text_bytes: usize,
    text_chunks: Vec<TextChunk>,
    xmp_packet: Option<String>,
    time: Option<String>,
}

struct TextChunk {
    keyword: String,
    text: String,
    chunk_type: &'static str,
    language: Option<String>,
}

struct PngPhys {
    x: u32,
    y: u32,
    unit: u8,
}
