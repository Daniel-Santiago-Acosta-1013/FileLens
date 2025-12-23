//! Extracción de metadata para audio y video.

use crate::advanced_metadata::AdvancedMetadataResult;
use crate::metadata::report::{EntryLevel, ReportEntry, ReportSection, SectionNotice};
use chrono::{Duration, NaiveDate};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MediaKind {
    Mp3,
    Wav,
    Flac,
    Ogg,
    Mp4,
    Mkv,
    Unknown,
}

pub fn extract_media_metadata(path: &Path) -> AdvancedMetadataResult {
    let kind = detect_media_kind(path);
    match kind {
        MediaKind::Mp3 => build_section("Metadata MP3", read_mp3_metadata(path)),
        MediaKind::Wav => build_section("Metadata WAV", read_wav_metadata(path)),
        MediaKind::Flac => build_section("Metadata FLAC", read_flac_metadata(path)),
        MediaKind::Ogg => build_section("Metadata OGG", read_ogg_metadata(path)),
        MediaKind::Mp4 => build_section("Metadata MP4/MOV", read_mp4_metadata(path)),
        MediaKind::Mkv => build_section("Metadata MKV", read_mkv_metadata(path)),
        MediaKind::Unknown => {
            let mut section = ReportSection::new("Metadata multimedia");
            section.notice = Some(SectionNotice::new(
                "Formato multimedia no reconocido",
                EntryLevel::Muted,
            ));
            AdvancedMetadataResult {
                section,
                risks: Vec::new(),
            }
        }
    }
}

fn build_section(title: &str, metadata: Option<Vec<ReportEntry>>) -> AdvancedMetadataResult {
    let mut section = ReportSection::new(title);
    let risks = Vec::new();
    if let Some(entries) = metadata {
        section.entries = entries;
    } else {
        section.notice = Some(SectionNotice::new(
            "No se pudo leer metadata multimedia",
            EntryLevel::Warning,
        ));
    }
    AdvancedMetadataResult { section, risks }
}

fn detect_media_kind(path: &Path) -> MediaKind {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return MediaKind::Unknown,
    };
    let mut header = [0_u8; 12];
    let _ = file.read(&mut header);
    if header.starts_with(b"ID3") {
        return MediaKind::Mp3;
    }
    if header.starts_with(b"RIFF") && &header[8..12] == b"WAVE" {
        return MediaKind::Wav;
    }
    if header.starts_with(b"fLaC") {
        return MediaKind::Flac;
    }
    if header.starts_with(b"OggS") {
        return MediaKind::Ogg;
    }
    if header.len() >= 12 && &header[4..8] == b"ftyp" {
        return MediaKind::Mp4;
    }
    if header.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return MediaKind::Mkv;
    }
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_lowercase().as_str() {
        "mp3" => MediaKind::Mp3,
        "wav" => MediaKind::Wav,
        "flac" => MediaKind::Flac,
        "ogg" | "opus" => MediaKind::Ogg,
        "mp4" | "m4a" | "mov" => MediaKind::Mp4,
        "mkv" => MediaKind::Mkv,
        _ => MediaKind::Unknown,
    }
}

// === MP3 ===

fn read_mp3_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let file_size = file.metadata().ok()?.len();
    let mut entries = Vec::new();

    let (id3, audio_offset) = parse_id3v2(&mut file).unwrap_or((Id3Data::default(), 0));
    if let Some(version) = id3.version {
        entries.push(ReportEntry::info("ID3 versión", version));
    }
    if let Some(value) = id3.title {
        entries.push(ReportEntry::info("Título", value));
    }
    if let Some(value) = id3.artist {
        entries.push(ReportEntry::info("Artista", value));
    }
    if let Some(value) = id3.album {
        entries.push(ReportEntry::info("Álbum", value));
    }
    if let Some(value) = id3.year {
        entries.push(ReportEntry::info("Año/Fecha", value));
    }
    if let Some(value) = id3.track {
        entries.push(ReportEntry::info("Track", value));
    }
    if let Some(value) = id3.genre {
        entries.push(ReportEntry::info("Género", value));
    }
    if let Some(value) = id3.composer {
        entries.push(ReportEntry::info("Composer", value));
    }
    if let Some(value) = id3.publisher {
        entries.push(ReportEntry::info("Publisher", value));
    }
    if let Some(value) = id3.comments {
        entries.push(ReportEntry::info("Comentarios", value));
    }
    entries.push(ReportEntry::info(
        "Letras",
        if id3.has_lyrics { "Sí" } else { "No" },
    ));
    if let Some(cover) = id3.cover {
        entries.push(ReportEntry::info("Carátula", cover));
    }

    let header = read_mp3_frame_header(&mut file, audio_offset)?;
    entries.push(ReportEntry::info("MPEG versión", header.mpeg_version));
    entries.push(ReportEntry::info("Layer", header.layer));
    if let Some(bitrate) = header.bitrate_kbps {
        entries.push(ReportEntry::info("Bitrate", format!("{bitrate} kbps")));
    }
    if let Some(rate) = header.sample_rate {
        entries.push(ReportEntry::info("Sample rate", format!("{rate} Hz")));
    }
    entries.push(ReportEntry::info("Channels", header.channels));
    entries.push(ReportEntry::info(
        "Padding",
        if header.padding { "Sí" } else { "No" },
    ));

    let scan = scan_mp3_headers(&mut file, audio_offset);
    if let Some(vbr) = scan.vbr {
        entries.push(ReportEntry::info("VBR/CBR", vbr));
    } else {
        entries.push(ReportEntry::info("VBR/CBR", "Desconocido"));
    }
    if let Some(encoder) = scan.encoder {
        entries.push(ReportEntry::info("Encoder", encoder));
    }
    if let Some(frames) = scan.frame_count {
        entries.push(ReportEntry::info("Frame count", frames.to_string()));
    }

    if let Some(bitrate) = header.bitrate_kbps {
        let audio_size = file_size.saturating_sub(audio_offset);
        let duration = (audio_size as f64 * 8.0) / (bitrate as f64 * 1000.0);
        entries.push(ReportEntry::info(
            "Duración",
            format!("{duration:.2} s"),
        ));
    }

    Some(entries)
}

#[derive(Default)]
struct Id3Data {
    version: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    year: Option<String>,
    track: Option<String>,
    genre: Option<String>,
    composer: Option<String>,
    publisher: Option<String>,
    comments: Option<String>,
    has_lyrics: bool,
    cover: Option<String>,
}

struct Mp3Scan {
    vbr: Option<&'static str>,
    encoder: Option<String>,
    frame_count: Option<u32>,
}

fn parse_id3v2(file: &mut File) -> Option<(Id3Data, u64)> {
    let mut header = [0_u8; 10];
    file.read_exact(&mut header).ok()?;
    if &header[0..3] != b"ID3" {
        file.seek(SeekFrom::Start(0)).ok()?;
        return None;
    }
    let version = format!("v2.{}.{}", header[3], header[4]);
    let size = synchsafe_to_u32(&header[6..10]) as u64;
    let mut tag_data = vec![0_u8; size as usize];
    file.read_exact(&mut tag_data).ok()?;
    let mut data = Id3Data::default();
    data.version = Some(version);
    let mut offset = 0;
    while offset + 10 <= tag_data.len() {
        let frame_id = &tag_data[offset..offset + 4];
        if frame_id.iter().all(|b| *b == 0) {
            break;
        }
        let frame_size = u32::from_be_bytes([
            tag_data[offset + 4],
            tag_data[offset + 5],
            tag_data[offset + 6],
            tag_data[offset + 7],
        ]) as usize;
        let frame_start = offset + 10;
        let frame_end = frame_start + frame_size;
        if frame_end > tag_data.len() {
            break;
        }
        let frame = &tag_data[frame_start..frame_end];
        match frame_id {
            b"TIT2" => data.title = decode_id3_text(frame),
            b"TPE1" => data.artist = decode_id3_text(frame),
            b"TALB" => data.album = decode_id3_text(frame),
            b"TDRC" | b"TYER" => data.year = decode_id3_text(frame),
            b"TRCK" => data.track = decode_id3_text(frame),
            b"TCON" => data.genre = decode_id3_text(frame),
            b"TCOM" => data.composer = decode_id3_text(frame),
            b"TPUB" => data.publisher = decode_id3_text(frame),
            b"COMM" => data.comments = decode_id3_text(frame),
            b"USLT" => data.has_lyrics = true,
            b"APIC" => data.cover = parse_apic(frame),
            _ => {}
        }
        offset = frame_end;
    }
    let audio_offset = 10 + size;
    Some((data, audio_offset))
}

fn scan_mp3_headers(file: &mut File, offset: u64) -> Mp3Scan {
    let mut buffer = vec![0_u8; 4096];
    let _ = file.seek(SeekFrom::Start(offset));
    let bytes = file.read(&mut buffer).unwrap_or(0);
    buffer.truncate(bytes);

    let (vbr, frame_count) = detect_xing_header(&buffer);
    let encoder = detect_mp3_encoder(&buffer);

    Mp3Scan {
        vbr,
        encoder,
        frame_count,
    }
}

fn detect_xing_header(data: &[u8]) -> (Option<&'static str>, Option<u32>) {
    if let Some(idx) = find_bytes(data, b"Xing") {
        return (Some("VBR"), parse_xing_frames(data, idx));
    }
    if let Some(idx) = find_bytes(data, b"Info") {
        return (Some("CBR"), parse_xing_frames(data, idx));
    }
    (None, None)
}

fn parse_xing_frames(data: &[u8], idx: usize) -> Option<u32> {
    if idx + 8 > data.len() {
        return None;
    }
    let flags = u32::from_be_bytes([data[idx + 4], data[idx + 5], data[idx + 6], data[idx + 7]]);
    if flags & 0x1 == 0 {
        return None;
    }
    if idx + 12 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[idx + 8],
        data[idx + 9],
        data[idx + 10],
        data[idx + 11],
    ]))
}

fn detect_mp3_encoder(data: &[u8]) -> Option<String> {
    if let Some(idx) = find_bytes(data, b"LAME") {
        return Some(read_tag_label(data, idx, 12).unwrap_or_else(|| "LAME".to_string()));
    }
    if let Some(idx) = find_bytes(data, b"Lavf") {
        return Some(read_tag_label(data, idx, 12).unwrap_or_else(|| "Lavf".to_string()));
    }
    if let Some(idx) = find_bytes(data, b"iTunes") {
        return Some(read_tag_label(data, idx, 12).unwrap_or_else(|| "iTunes".to_string()));
    }
    if let Some(idx) = find_bytes(data, b"FhG") {
        return Some(read_tag_label(data, idx, 12).unwrap_or_else(|| "FhG".to_string()));
    }
    None
}

fn read_tag_label(data: &[u8], start: usize, max: usize) -> Option<String> {
    let end = (start + max).min(data.len());
    let mut label = String::new();
    for &b in &data[start..end] {
        if !b.is_ascii_graphic() && b != b' ' {
            break;
        }
        label.push(b as char);
    }
    if label.trim().is_empty() {
        None
    } else {
        Some(label.trim().to_string())
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn synchsafe_to_u32(bytes: &[u8]) -> u32 {
    let mut value = 0_u32;
    for &b in bytes {
        value = (value << 7) | (b as u32 & 0x7F);
    }
    value
}

fn decode_id3_text(frame: &[u8]) -> Option<String> {
    if frame.is_empty() {
        return None;
    }
    let encoding = frame[0];
    let data = &frame[1..];
    match encoding {
        0 => Some(String::from_utf8_lossy(data).trim().to_string()),
        1 | 2 => {
            if data.len() < 2 {
                return None;
            }
            let utf16 = data
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            Some(String::from_utf16_lossy(&utf16).trim().to_string())
        }
        3 => Some(String::from_utf8_lossy(data).trim().to_string()),
        _ => None,
    }
}

fn parse_apic(frame: &[u8]) -> Option<String> {
    if frame.len() < 4 {
        return None;
    }
    let mut pos = 1;
    while pos < frame.len() && frame[pos] != 0 {
        pos += 1;
    }
    let mime = String::from_utf8_lossy(&frame[1..pos]).to_string();
    let size = frame.len();
    Some(format!("{mime} ({size} bytes)"))
}

struct Mp3FrameHeader {
    mpeg_version: String,
    layer: String,
    bitrate_kbps: Option<u32>,
    sample_rate: Option<u32>,
    channels: String,
    padding: bool,
}

fn read_mp3_frame_header(file: &mut File, offset: u64) -> Option<Mp3FrameHeader> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buffer = [0_u8; 4];
    loop {
        if file.read_exact(&mut buffer).is_err() {
            return None;
        }
        if buffer[0] == 0xFF && buffer[1] & 0xE0 == 0xE0 {
            break;
        }
        file.seek(SeekFrom::Current(-3)).ok()?;
    }
    let header = u32::from_be_bytes(buffer);
    let version_bits = (header >> 19) & 0x3;
    let layer_bits = (header >> 17) & 0x3;
    let bitrate_index = (header >> 12) & 0xF;
    let sample_index = (header >> 10) & 0x3;
    let padding = ((header >> 9) & 0x1) != 0;
    let channel_mode = (header >> 6) & 0x3;

    let (mpeg_version, sample_rate) = match version_bits {
        0b11 => ("MPEG1", mp3_sample_rate(sample_index, 44100, 48000, 32000)),
        0b10 => ("MPEG2", mp3_sample_rate(sample_index, 22050, 24000, 16000)),
        0b00 => ("MPEG2.5", mp3_sample_rate(sample_index, 11025, 12000, 8000)),
        _ => ("Desconocido", None),
    };
    let layer = match layer_bits {
        0b01 => "Layer III",
        0b10 => "Layer II",
        0b11 => "Layer I",
        _ => "Desconocido",
    };
    let bitrate_kbps = mp3_bitrate(layer_bits, version_bits, bitrate_index);
    let channels = match channel_mode {
        0 => "Stereo",
        1 => "Joint Stereo",
        2 => "Dual",
        3 => "Mono",
        _ => "Desconocido",
    };

    Some(Mp3FrameHeader {
        mpeg_version: mpeg_version.to_string(),
        layer: layer.to_string(),
        bitrate_kbps,
        sample_rate,
        channels: channels.to_string(),
        padding,
    })
}

fn mp3_sample_rate(index: u32, a: u32, b: u32, c: u32) -> Option<u32> {
    match index {
        0 => Some(a),
        1 => Some(b),
        2 => Some(c),
        _ => None,
    }
}

fn mp3_bitrate(layer_bits: u32, version_bits: u32, index: u32) -> Option<u32> {
    if index == 0 || index == 0xF {
        return None;
    }
    let table = match (version_bits, layer_bits) {
        (0b11, 0b01) => [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0],
        (0b11, 0b10) => [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0],
        (0b11, 0b11) => [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0],
        _ => [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
    };
    Some(table[index as usize])
}

// === WAV ===

fn read_wav_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let mut entries = Vec::new();
    let mut chunks = Vec::new();
    let mut duration = None;
    let mut byte_rate = None;
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
            "fmt " => {
                let mut payload = vec![0_u8; size.min(16)];
                file.read_exact(&mut payload).ok()?;
                if payload.len() >= 16 {
                    let format = u16::from_le_bytes([payload[0], payload[1]]);
                    let channels = u16::from_le_bytes([payload[2], payload[3]]);
                    let sample_rate = u32::from_le_bytes([
                        payload[4],
                        payload[5],
                        payload[6],
                        payload[7],
                    ]);
                    let br = u32::from_le_bytes([
                        payload[8],
                        payload[9],
                        payload[10],
                        payload[11],
                    ]);
                    let block_align = u16::from_le_bytes([payload[12], payload[13]]);
                    let bits_per_sample = u16::from_le_bytes([payload[14], payload[15]]);
                    byte_rate = Some(br);
                    entries.push(ReportEntry::info(
                        "Audio format",
                        format.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Channels",
                        channels.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Sample rate",
                        sample_rate.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Byte rate",
                        br.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Block align",
                        block_align.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Bits por muestra",
                        bits_per_sample.to_string(),
                    ));
                }
                if size > payload.len() {
                    let _ = file.seek(SeekFrom::Current((size - payload.len()) as i64));
                }
            }
            "data" => {
                if let Some(br) = byte_rate {
                    duration = Some(size as f64 / br as f64);
                }
                let _ = file.seek(SeekFrom::Current(size as i64));
            }
            "LIST" => {
                let mut payload = vec![0_u8; size.min(512)];
                let _ = file.read_exact(&mut payload);
                if payload.starts_with(b"INFO") {
                    entries.push(ReportEntry::info("INFO", "Detectado"));
                }
                if size > payload.len() {
                    let _ = file.seek(SeekFrom::Current((size - payload.len()) as i64));
                }
            }
            "bext" => {
                let mut payload = vec![0_u8; size.min(602)];
                let _ = file.read_exact(&mut payload);
                if payload.len() >= 602 {
                    let description = read_ascii_field(&payload, 0, 256);
                    let originator = read_ascii_field(&payload, 256, 32);
                    let originator_ref = read_ascii_field(&payload, 288, 32);
                    let orig_date = read_ascii_field(&payload, 320, 10);
                    let orig_time = read_ascii_field(&payload, 330, 8);
                    let time_ref = u64::from_le_bytes([
                        payload[338],
                        payload[339],
                        payload[340],
                        payload[341],
                        payload[342],
                        payload[343],
                        payload[344],
                        payload[345],
                    ]);
                    let version = u16::from_le_bytes([payload[346], payload[347]]);
                    if !description.is_empty() {
                        entries.push(ReportEntry::info("BEXT Descripcion", description));
                    }
                    if !originator.is_empty() {
                        entries.push(ReportEntry::info("BEXT Originator", originator));
                    }
                    if !originator_ref.is_empty() {
                        entries.push(ReportEntry::info("BEXT Originator ref", originator_ref));
                    }
                    if !orig_date.is_empty() {
                        entries.push(ReportEntry::info("BEXT Fecha", orig_date));
                    }
                    if !orig_time.is_empty() {
                        entries.push(ReportEntry::info("BEXT Hora", orig_time));
                    }
                    entries.push(ReportEntry::info("BEXT Time ref", time_ref.to_string()));
                    entries.push(ReportEntry::info("BEXT Version", version.to_string()));
                } else {
                    let desc = String::from_utf8_lossy(&payload).trim().to_string();
                    if !desc.is_empty() {
                        entries.push(ReportEntry::info("BEXT Descripcion", desc));
                    }
                }
                if size > payload.len() {
                    let _ = file.seek(SeekFrom::Current((size - payload.len()) as i64));
                }
            }
            _ => {
                let _ = file.seek(SeekFrom::Current(size as i64));
            }
        }
        if size % 2 == 1 {
            let _ = file.seek(SeekFrom::Current(1));
        }
    }

    if !chunks.is_empty() {
        entries.push(ReportEntry::info(
            "Chunks presentes",
            chunks.join(", "),
        ));
    }
    if let Some(duration) = duration {
        entries.push(ReportEntry::info(
            "Duración",
            format!("{duration:.2} s"),
        ));
    }
    Some(entries)
}

// === FLAC ===

fn read_flac_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let mut signature = [0_u8; 4];
    file.read_exact(&mut signature).ok()?;
    if &signature != b"fLaC" {
        return None;
    }
    let mut entries = Vec::new();
    let mut is_last = false;
    let mut vendor = None;
    let mut comments = HashMap::new();
    while !is_last {
        let mut header = [0_u8; 4];
        file.read_exact(&mut header).ok()?;
        is_last = header[0] & 0x80 != 0;
        let block_type = header[0] & 0x7F;
        let length = ((header[1] as usize) << 16) | ((header[2] as usize) << 8) | header[3] as usize;
        let mut payload = vec![0_u8; length];
        file.read_exact(&mut payload).ok()?;
        match block_type {
            0 => {
                if payload.len() >= 34 {
                    let sample_rate = ((payload[10] as u32) << 12)
                        | ((payload[11] as u32) << 4)
                        | ((payload[12] as u32) >> 4);
                    let channels = ((payload[12] >> 1) & 0x07) + 1;
                    let bits_per_sample = (((payload[12] & 0x01) as u16) << 4)
                        | ((payload[13] as u16) >> 4);
                    let total_samples = ((payload[13] as u64 & 0x0F) << 32)
                        | ((payload[14] as u64) << 24)
                        | ((payload[15] as u64) << 16)
                        | ((payload[16] as u64) << 8)
                        | payload[17] as u64;
                    let duration = if sample_rate > 0 {
                        total_samples as f64 / sample_rate as f64
                    } else {
                        0.0
                    };
                    entries.push(ReportEntry::info(
                        "Sample rate",
                        sample_rate.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Channels",
                        channels.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Bits por muestra",
                        bits_per_sample.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Total samples",
                        total_samples.to_string(),
                    ));
                    entries.push(ReportEntry::info(
                        "Duración",
                        format!("{duration:.2} s"),
                    ));
                    if payload.len() >= 34 {
                        let md5 = payload[18..34]
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>();
                        entries.push(ReportEntry::info("MD5 audio", md5));
                    }
                }
            }
            4 => {
                let mut cursor = &payload[..];
                let vendor_len = read_u32_le(&mut cursor) as usize;
                if cursor.len() >= vendor_len {
                    vendor = Some(String::from_utf8_lossy(&cursor[..vendor_len]).to_string());
                    cursor = &cursor[vendor_len..];
                }
                let count = read_u32_le(&mut cursor);
                for _ in 0..count {
                    let len = read_u32_le(&mut cursor) as usize;
                    if cursor.len() < len {
                        break;
                    }
                    let entry = String::from_utf8_lossy(&cursor[..len]).to_string();
                    cursor = &cursor[len..];
                    if let Some((k, v)) = entry.split_once('=') {
                        comments.insert(k.to_string(), v.to_string());
                    }
                }
            }
            6 => {
                entries.push(ReportEntry::info("PICTURE", "Detectado"));
            }
            _ => {}
        }
    }
    if let Some(vendor) = vendor {
        entries.push(ReportEntry::info("Vendor", vendor));
    }
    for (key, value) in comments {
        entries.push(ReportEntry::info(format!("TAG {key}"), value));
    }
    Some(entries)
}

// === OGG ===

fn read_ogg_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    if !data.starts_with(b"OggS") {
        return None;
    }
    let mut entries = Vec::new();
    let mut codec = "Desconocido";
    let mut sample_rate = None;
    let mut channels = None;
    let mut vendor = None;
    let mut tags = HashMap::new();
    let mut granule_position = 0_u64;
    let mut pages = 0;
    let mut serial = None;
    let mut offset = 0;
    while offset + 27 <= data.len() {
        if &data[offset..offset + 4] != b"OggS" {
            break;
        }
        pages += 1;
        if serial.is_none() {
            serial = Some(u32::from_le_bytes([
                data[offset + 14],
                data[offset + 15],
                data[offset + 16],
                data[offset + 17],
            ]));
        }
        let gp = u64::from_le_bytes([
            data[offset + 6],
            data[offset + 7],
            data[offset + 8],
            data[offset + 9],
            data[offset + 10],
            data[offset + 11],
            data[offset + 12],
            data[offset + 13],
        ]);
        granule_position = gp;
        let segments = data[offset + 26] as usize;
        let seg_table_start = offset + 27;
        let seg_table_end = seg_table_start + segments;
        if seg_table_end > data.len() {
            break;
        }
        let mut total = 0usize;
        for i in 0..segments {
            total += data[seg_table_start + i] as usize;
        }
        let packet_start = seg_table_end;
        let packet_end = packet_start + total;
        if packet_end > data.len() {
            break;
        }
        let packet = &data[packet_start..packet_end];
        if packet.starts_with(b"OpusHead") {
            codec = "Opus";
            channels = packet.get(9).map(|b| *b as u16);
            sample_rate = Some(48_000);
        } else if packet.len() > 7 && packet[0] == 0x01 && &packet[1..7] == b"vorbis" {
            codec = "Vorbis";
            channels = packet.get(11).map(|b| *b as u16);
            sample_rate = packet.get(12..16).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        } else if packet.len() > 7 && packet[0] == 0x03 && &packet[1..7] == b"vorbis" {
            let mut cursor = &packet[7..];
            let vendor_len = read_u32_le(&mut cursor) as usize;
            if cursor.len() >= vendor_len {
                vendor = Some(String::from_utf8_lossy(&cursor[..vendor_len]).to_string());
                cursor = &cursor[vendor_len..];
            }
            let count = read_u32_le(&mut cursor);
            for _ in 0..count {
                let len = read_u32_le(&mut cursor) as usize;
                if cursor.len() < len {
                    break;
                }
                let entry = String::from_utf8_lossy(&cursor[..len]).to_string();
                cursor = &cursor[len..];
                if let Some((k, v)) = entry.split_once('=') {
                    tags.insert(k.to_string(), v.to_string());
                }
            }
        }
        offset = packet_end;
    }
    entries.push(ReportEntry::info("Codec", codec));
    if let Some(rate) = sample_rate {
        entries.push(ReportEntry::info("Sample rate", rate.to_string()));
    }
    if let Some(ch) = channels {
        entries.push(ReportEntry::info("Channels", ch.to_string()));
    }
    if let Some(vendor) = vendor {
        entries.push(ReportEntry::info("Vendor", vendor));
    }
    if let Some(serial) = serial {
        entries.push(ReportEntry::info("Stream serial", serial.to_string()));
    }
    if let Some(rate) = sample_rate {
        let duration = granule_position as f64 / rate as f64;
        entries.push(ReportEntry::info("Duración", format!("{duration:.2} s")));
    }
    entries.push(ReportEntry::info("Páginas OGG", pages.to_string()));
    for (key, value) in tags {
        entries.push(ReportEntry::info(format!("TAG {key}"), value));
    }
    Some(entries)
}

// === MP4/MOV ===

fn read_mp4_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let mut entries = Vec::new();
    let mut moov_before_mdat = false;
    let mut brands = Vec::new();
    let mut duration = None;
    let mut timescale = None;
    let mut creation_time = None;
    let mut modification_time = None;
    let mut tracks = Vec::new();
    let mut mdat_seen = false;
    loop {
        let Some(header) = read_box_header(&mut file) else { break };
        let box_type = String::from_utf8_lossy(&header.kind).to_string();
        match box_type.as_str() {
            "ftyp" => {
                let payload = read_box_payload(&mut file, &header, 1024 * 1024)?;
                if payload.len() >= 8 {
                    let major = String::from_utf8_lossy(&payload[0..4]).to_string();
                    brands.push(major);
                    let mut offset = 8;
                    while offset + 4 <= payload.len() {
                        brands.push(String::from_utf8_lossy(&payload[offset..offset + 4]).to_string());
                        offset += 4;
                    }
                }
            }
            "moov" => {
                if !mdat_seen {
                    moov_before_mdat = true;
                }
                let payload = read_box_payload(&mut file, &header, 8 * 1024 * 1024)?;
                parse_mp4_moov(
                    &payload,
                    &mut duration,
                    &mut timescale,
                    &mut creation_time,
                    &mut modification_time,
                    &mut tracks,
                );
            }
            "mdat" => {
                mdat_seen = true;
                let _ = file.seek(SeekFrom::Current(header.payload_size as i64));
            }
            _ => {
                let _ = file.seek(SeekFrom::Current(header.payload_size as i64));
            }
        }
    }
    if !brands.is_empty() {
        entries.push(ReportEntry::info(
            "Brands",
            brands.join(", "),
        ));
    }
    if let (Some(duration), Some(timescale)) = (duration, timescale) {
        let seconds = duration as f64 / timescale as f64;
        entries.push(ReportEntry::info("Duración", format!("{seconds:.2} s")));
        entries.push(ReportEntry::info("Timescale", timescale.to_string()));
    }
    if let Some(value) = creation_time {
        entries.push(ReportEntry::info(
            "Creation time",
            format_mp4_time(value),
        ));
    }
    if let Some(value) = modification_time {
        entries.push(ReportEntry::info(
            "Modification time",
            format_mp4_time(value),
        ));
    }
    entries.push(ReportEntry::info(
        "Fast start",
        if moov_before_mdat { "Sí" } else { "No" },
    ));
    entries.push(ReportEntry::info(
        "Tracks",
        tracks.len().to_string(),
    ));
    for track in tracks {
        entries.push(ReportEntry::info("Track", track));
    }
    Some(entries)
}

fn parse_mp4_moov(
    data: &[u8],
    duration: &mut Option<u64>,
    timescale: &mut Option<u32>,
    creation_time: &mut Option<u64>,
    modification_time: &mut Option<u64>,
    tracks: &mut Vec<String>,
) {
    let mut cursor = Cursor::new(data);
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let payload = read_box_payload(&mut cursor, &header, 4 * 1024 * 1024).unwrap_or_default();
        match name.as_str() {
            "mvhd" => {
                if payload.len() >= 20 {
                    let version = payload[0];
                    if version == 1 && payload.len() >= 32 {
                        *creation_time = Some(u64::from_be_bytes([
                            payload[4], payload[5], payload[6], payload[7],
                            payload[8], payload[9], payload[10], payload[11],
                        ]));
                        *modification_time = Some(u64::from_be_bytes([
                            payload[12], payload[13], payload[14], payload[15],
                            payload[16], payload[17], payload[18], payload[19],
                        ]));
                        *timescale = Some(u32::from_be_bytes([payload[20], payload[21], payload[22], payload[23]]));
                        *duration = Some(u64::from_be_bytes([
                            payload[24], payload[25], payload[26], payload[27],
                            payload[28], payload[29], payload[30], payload[31],
                        ]));
                    } else if version == 0 && payload.len() >= 20 {
                        *creation_time = Some(u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]) as u64);
                        *modification_time = Some(u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]) as u64);
                        *timescale = Some(u32::from_be_bytes([payload[12], payload[13], payload[14], payload[15]]));
                        *duration = Some(u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]) as u64);
                    }
                }
            }
            "trak" => {
                if let Some(track_info) = parse_mp4_trak(&payload) {
                    tracks.push(track_info);
                }
            }
            _ => {}
        }
    }
}

fn parse_mp4_trak(data: &[u8]) -> Option<String> {
    let mut cursor = Cursor::new(data);
    let mut track_type = None;
    let mut codec = None;
    let mut track_duration = None;
    let mut dimensions = None;
    let mut audio = None;
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let payload = read_box_payload(&mut cursor, &header, 2 * 1024 * 1024).unwrap_or_default();
        match name.as_str() {
            "tkhd" => {
                if payload.len() >= 84 {
                    let width = u32::from_be_bytes([payload[76], payload[77], payload[78], payload[79]]) >> 16;
                    let height = u32::from_be_bytes([payload[80], payload[81], payload[82], payload[83]]) >> 16;
                    if width > 0 && height > 0 {
                        dimensions = Some(format!("{width}x{height}"));
                    }
                }
            }
            "mdia" => {
                if let Some((t, c, d, a)) = parse_mp4_mdia(&payload) {
                    track_type = t;
                    codec = c;
                    track_duration = d;
                    audio = a;
                }
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if let Some(track_type) = track_type {
        parts.push(format!("tipo:{track_type}"));
    }
    if let Some(codec) = codec {
        parts.push(format!("codec:{codec}"));
    }
    if let Some(duration) = track_duration {
        parts.push(format!("dur:{duration}"));
    }
    if let Some(dim) = dimensions {
        parts.push(format!("size:{dim}"));
    }
    if let Some(audio) = audio {
        parts.push(audio);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn parse_mp4_mdia(data: &[u8]) -> Option<(Option<String>, Option<String>, Option<String>, Option<String>)> {
    let mut cursor = Cursor::new(data);
    let mut track_type = None;
    let mut codec = None;
    let mut duration = None;
    let mut audio = None;
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let payload = read_box_payload(&mut cursor, &header, 2 * 1024 * 1024).unwrap_or_default();
        match name.as_str() {
            "hdlr" => {
                if payload.len() >= 16 {
                    let handler = String::from_utf8_lossy(&payload[8..12]).to_string();
                    track_type = Some(handler);
                }
            }
            "mdhd" => {
                if payload.len() >= 20 {
                    let version = payload[0];
                    if version == 0 {
                        let timescale = u32::from_be_bytes([payload[12], payload[13], payload[14], payload[15]]);
                        let dur = u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]);
                        duration = Some(format!("{:.2}s", dur as f64 / timescale as f64));
                    }
                }
            }
            "minf" => {
                if let Some((c, a)) = parse_mp4_minf(&payload) {
                    codec = c;
                    audio = a;
                }
            }
            _ => {}
        }
    }
    Some((track_type, codec, duration, audio))
}

fn parse_mp4_minf(data: &[u8]) -> Option<(Option<String>, Option<String>)> {
    let mut cursor = Cursor::new(data);
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let payload = read_box_payload(&mut cursor, &header, 2 * 1024 * 1024).unwrap_or_default();
        if name == "stbl" {
            return parse_mp4_stbl(&payload);
        }
    }
    None
}

fn parse_mp4_stbl(data: &[u8]) -> Option<(Option<String>, Option<String>)> {
    let mut cursor = Cursor::new(data);
    while let Some(header) = read_box_header(&mut cursor) {
        let name = String::from_utf8_lossy(&header.kind).to_string();
        let payload = read_box_payload(&mut cursor, &header, 2 * 1024 * 1024).unwrap_or_default();
        if name == "stsd" && payload.len() >= 16 {
            let entry_type = String::from_utf8_lossy(&payload[12..16]).to_string();
            let audio = if payload.len() >= 36 {
                let channel_count = u16::from_be_bytes([payload[24], payload[25]]);
                let sample_rate = u32::from_be_bytes([payload[32], payload[33], payload[34], payload[35]]) >> 16;
                Some(format!("audio:{channel_count}ch {sample_rate}Hz"))
            } else {
                None
            };
            return Some((Some(entry_type), audio));
        }
    }
    None
}

// === MKV ===

fn read_mkv_metadata(path: &Path) -> Option<Vec<ReportEntry>> {
    let mut file = File::open(path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;
    if data.len() < 4 || &data[0..4] != [0x1A, 0x45, 0xDF, 0xA3] {
        return None;
    }
    let mut entries = Vec::new();
    entries.push(ReportEntry::info("EBML", "Detectado"));
    let mut cursor = Cursor::new(data.as_slice());
    while let Some((id, size)) = read_ebml_element(&mut cursor) {
        let start = cursor.position() as usize;
        let end = start + size as usize;
        if end > data.len() {
            break;
        }
        if id == 0x1A45DFA3 {
            parse_mkv_ebml_header(&data[start..end], &mut entries);
        } else if id == 0x1549A966 {
            parse_mkv_info(&data[start..end], &mut entries);
        } else if id == 0x1654AE6B {
            parse_mkv_tracks(&data[start..end], &mut entries);
        }
        cursor.set_position(end as u64);
    }
    Some(entries)
}

fn parse_mkv_info(data: &[u8], entries: &mut Vec<ReportEntry>) {
    let mut cursor = Cursor::new(data);
    while let Some((id, size)) = read_ebml_element(&mut cursor) {
        let start = cursor.position() as usize;
        let end = start + size as usize;
        if end > data.len() {
            break;
        }
        match id {
            0x4D80 => entries.push(ReportEntry::info(
                "Muxing app",
                read_ebml_string(&data[start..end]),
            )),
            0x5741 => entries.push(ReportEntry::info(
                "Writing app",
                read_ebml_string(&data[start..end]),
            )),
            0x2AD7B1 => entries.push(ReportEntry::info(
                "Timecode scale",
                read_ebml_uint(&data[start..end]).to_string(),
            )),
            0x4489 => entries.push(ReportEntry::info(
                "Duración",
                read_ebml_float(&data[start..end]).map(|d| format!("{d:.2}")).unwrap_or_else(|| "N/D".to_string()),
            )),
            _ => {}
        }
        cursor.set_position(end as u64);
    }
}

fn parse_mkv_tracks(data: &[u8], entries: &mut Vec<ReportEntry>) {
    let mut cursor = Cursor::new(data);
    let mut tracks = 0;
    while let Some((id, size)) = read_ebml_element(&mut cursor) {
        let start = cursor.position() as usize;
        let end = start + size as usize;
        if end > data.len() {
            break;
        }
        if id == 0xAE {
            tracks += 1;
            let detail = parse_mkv_track_entry(&data[start..end]);
            let label = if let Some(detail) = detail {
                detail
            } else {
                format!("Track {tracks}")
            };
            entries.push(ReportEntry::info("Track", label));
        }
        cursor.set_position(end as u64);
    }
}

fn parse_mkv_ebml_header(data: &[u8], entries: &mut Vec<ReportEntry>) {
    let mut cursor = Cursor::new(data);
    while let Some((id, size)) = read_ebml_element(&mut cursor) {
        let start = cursor.position() as usize;
        let end = start + size as usize;
        if end > data.len() {
            break;
        }
        match id {
            0x4286 => entries.push(ReportEntry::info(
                "EBML version",
                read_ebml_uint(&data[start..end]).to_string(),
            )),
            0x4282 => entries.push(ReportEntry::info(
                "Doc type",
                read_ebml_string(&data[start..end]),
            )),
            _ => {}
        }
        cursor.set_position(end as u64);
    }
}

fn parse_mkv_track_entry(data: &[u8]) -> Option<String> {
    let mut cursor = Cursor::new(data);
    let mut track_number = None;
    let mut track_type = None;
    let mut codec_id = None;
    let mut codec_name = None;
    let mut language = None;
    let mut default_flag = None;
    let mut forced_flag = None;
    while let Some((id, size)) = read_ebml_element(&mut cursor) {
        let start = cursor.position() as usize;
        let end = start + size as usize;
        if end > data.len() {
            break;
        }
        match id {
            0xD7 => track_number = Some(read_ebml_uint(&data[start..end])),
            0x83 => track_type = Some(read_ebml_uint(&data[start..end])),
            0x86 => codec_id = Some(read_ebml_string(&data[start..end])),
            0x258688 => codec_name = Some(read_ebml_string(&data[start..end])),
            0x22B59C => language = Some(read_ebml_string(&data[start..end])),
            0x88 => default_flag = Some(read_ebml_uint(&data[start..end]) != 0),
            0x55AA => forced_flag = Some(read_ebml_uint(&data[start..end]) != 0),
            _ => {}
        }
        cursor.set_position(end as u64);
    }
    let mut parts = Vec::new();
    if let Some(num) = track_number {
        parts.push(format!("id:{num}"));
    }
    if let Some(track_type) = track_type {
        parts.push(format!("tipo:{}", mkv_track_type_label(track_type)));
    }
    if let Some(codec_id) = codec_id {
        parts.push(format!("codec:{codec_id}"));
    }
    if let Some(codec_name) = codec_name {
        parts.push(format!("codec_name:{codec_name}"));
    }
    if let Some(language) = language {
        parts.push(format!("lang:{language}"));
    }
    if let Some(default_flag) = default_flag {
        parts.push(format!("default:{}", if default_flag { "si" } else { "no" }));
    }
    if let Some(forced_flag) = forced_flag {
        parts.push(format!("forced:{}", if forced_flag { "si" } else { "no" }));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn mkv_track_type_label(value: u64) -> &'static str {
    match value {
        1 => "video",
        2 => "audio",
        17 => "subtitles",
        _ => "otro",
    }
}

// === Helpers ===

fn read_u32_le(cursor: &mut &[u8]) -> u32 {
    if cursor.len() < 4 {
        return 0;
    }
    let value = u32::from_le_bytes([cursor[0], cursor[1], cursor[2], cursor[3]]);
    *cursor = &cursor[4..];
    value
}

fn read_ascii_field(data: &[u8], start: usize, len: usize) -> String {
    if start >= data.len() {
        return String::new();
    }
    let end = (start + len).min(data.len());
    String::from_utf8_lossy(&data[start..end])
        .trim_matches('\0')
        .trim()
        .to_string()
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
    let payload_size = size.saturating_sub(8);
    Some(BoxHeader { kind, payload_size })
}

fn read_box_payload<R: Read>(reader: &mut R, header: &BoxHeader, limit: usize) -> Option<Vec<u8>> {
    let size = header.payload_size as usize;
    if size > limit {
        let mut buffer = vec![0_u8; limit];
        reader.read_exact(&mut buffer).ok()?;
        let remaining = size.saturating_sub(limit);
        let _ = reader.by_ref().take(remaining as u64).read_to_end(&mut Vec::new());
        return Some(buffer);
    }
    let mut buffer = vec![0_u8; size];
    reader.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn read_ebml_element(cursor: &mut Cursor<&[u8]>) -> Option<(u32, u64)> {
    let id = read_ebml_id(cursor)?;
    let size = read_ebml_size(cursor)?;
    Some((id, size))
}

fn read_ebml_id(cursor: &mut Cursor<&[u8]>) -> Option<u32> {
    let mut first = [0_u8; 1];
    cursor.read_exact(&mut first).ok()?;
    let mut mask = 0x80;
    let mut length = 1;
    while length <= 8 && first[0] & mask == 0 {
        mask >>= 1;
        length += 1;
    }
    let mut value = first[0] as u32;
    for _ in 1..length {
        let mut b = [0_u8; 1];
        cursor.read_exact(&mut b).ok()?;
        value = (value << 8) | b[0] as u32;
    }
    Some(value)
}

fn read_ebml_size(cursor: &mut Cursor<&[u8]>) -> Option<u64> {
    let mut first = [0_u8; 1];
    cursor.read_exact(&mut first).ok()?;
    let mut mask = 0x80;
    let mut length = 1;
    while length <= 8 && first[0] & mask == 0 {
        mask >>= 1;
        length += 1;
    }
    let mut value = (first[0] & (!mask)) as u64;
    for _ in 1..length {
        let mut b = [0_u8; 1];
        cursor.read_exact(&mut b).ok()?;
        value = (value << 8) | b[0] as u64;
    }
    Some(value)
}

fn read_ebml_uint(data: &[u8]) -> u64 {
    let mut value = 0u64;
    for &b in data {
        value = (value << 8) | b as u64;
    }
    value
}

fn read_ebml_string(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim().to_string()
}

fn read_ebml_float(data: &[u8]) -> Option<f64> {
    match data.len() {
        4 => Some(f32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64),
        8 => Some(f64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ])),
        _ => None,
    }
}

fn format_mp4_time(seconds: u64) -> String {
    let Some(date) = NaiveDate::from_ymd_opt(1904, 1, 1) else {
        return seconds.to_string();
    };
    let Some(epoch) = date.and_hms_opt(0, 0, 0) else {
        return seconds.to_string();
    };
    let dt = epoch + Duration::seconds(seconds as i64);
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}
