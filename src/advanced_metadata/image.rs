//! Extracción de metadata de imágenes (EXIF, PNG, XMP/IPTC).

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use exif::{In, Tag};
use image::ImageReader;
use png::text_metadata::{ITXtChunk, ZTXtChunk};
use png::Decoder as PngDecoder;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use super::icc::extract_icc_profile;
use super::xmp::parse_xmp_metadata;

const SIDECAR_SCAN_LIMIT: u64 = 2 * 1024 * 1024; // 2 MiB
const TEXT_DECOMPRESS_LIMIT: usize = 2 * 1024 * 1024; // 2 MiB
const IFD_EXIF: In = In(2);
const IFD_GPS: In = In(3);
const IFD_INTEROP: In = In(4);

pub fn extract_image_metadata(path: &Path) -> AdvancedMetadataResult {
    let mut section = ReportSection::new("Metadata de imagen");
    let mut risks = Vec::new();
    let mut seen = HashSet::new();

    let mut has_entries = false;
    let mut xmp_detected = false;
    let mut xmp_parsed = false;

    if let Some(exif) = read_exif(path) {
        has_entries |= append_exif_entries(&mut section, &mut risks, &mut seen, &exif);
    }

    let mut dimensions = read_image_dimensions(path);

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

    if detect_iptc(path) {
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
        ExifSpec::info(Tag::FNumber, "Apertura"),
        ExifSpec::info(Tag::ExposureTime, "Exposición"),
        ExifSpec::info(Tag::PhotographicSensitivity, "ISO"),
        ExifSpec::info(Tag::ISOSpeed, "ISO"),
        ExifSpec::info(Tag::FocalLength, "Distancia focal"),
        ExifSpec::info(Tag::Orientation, "Orientación"),
        ExifSpec::info(Tag::XResolution, "Resolución X"),
        ExifSpec::info(Tag::YResolution, "Resolución Y"),
        ExifSpec::info(Tag::ResolutionUnit, "Unidad de resolución"),
        ExifSpec::info(Tag::LensMake, "Fabricante de lente"),
        ExifSpec::info(Tag::LensModel, "Modelo de lente"),
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

    if let Some(value) = gps_value(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef)
        && push_entry_unique(
            section,
            seen,
            ReportEntry::warning("GPS Latitud", &value),
        )
    {
        risks.push(ReportEntry::warning("GPS Latitud", value));
        has_entries = true;
    }
    if let Some(value) = gps_value(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef)
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
    let ref_field = exif.get_field(ref_tag, IFD_GPS)?;
    Some(format!(
        "{} {}",
        field.display_value(),
        ref_field.display_value()
    ))
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

        let label = format!("Texto ({})", chunk.keyword);
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

    let mut text_chunks = Vec::new();
    let mut xmp_packet = None;

    for chunk in &info.uncompressed_latin1_text {
        let keyword = chunk.keyword.clone();
        let text = chunk.text.clone();
        if is_xmp_keyword(&keyword) {
            xmp_packet = Some(text);
            continue;
        }
        text_chunks.push(TextChunk { keyword, text });
    }

    for chunk in &info.compressed_latin1_text {
        let keyword = chunk.keyword.clone();
        if let Some(text) = decode_ztxt(chunk) {
            if is_xmp_keyword(&keyword) {
                xmp_packet = Some(text);
                continue;
            }
            text_chunks.push(TextChunk { keyword, text });
        }
    }

    for chunk in &info.utf8_text {
        let keyword = chunk.keyword.clone();
        if let Some(text) = decode_itxt(chunk) {
            if is_xmp_keyword(&keyword) {
                xmp_packet = Some(text);
                continue;
            }
            text_chunks.push(TextChunk { keyword, text });
        }
    }

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
        text_chunks,
        xmp_packet,
    })
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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
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
    text_chunks: Vec<TextChunk>,
    xmp_packet: Option<String>,
}

struct TextChunk {
    keyword: String,
    text: String,
}
