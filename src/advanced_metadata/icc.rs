use crate::metadata::report::ReportEntry;
use std::collections::HashMap;

pub fn extract_icc_profile(profile: &[u8]) -> Vec<ReportEntry> {
    let mut entries = Vec::new();
    if profile.len() < 128 {
        return entries;
    }

    if let Some(value) = read_signature(profile, 4).map(map_vendor) {
        push(&mut entries, "Tipo perfil CMM", value);
    }
    if let Some(value) = read_profile_version(profile) {
        push(&mut entries, "Versión del perfil", value);
    }
    if let Some(value) = read_signature(profile, 12).map(map_device_class) {
        push(&mut entries, "Clase de perfil", value);
    }
    if let Some(value) = read_signature(profile, 16).map(map_color_space) {
        push(&mut entries, "Datos del espacio de color", value);
    }
    if let Some(value) = read_signature(profile, 20).map(map_color_space) {
        push(&mut entries, "Espacio de conexión del perfil", value);
    }
    if let Some(value) = read_icc_datetime(profile, 24) {
        push(&mut entries, "Fecha/Hora del perfil", value);
    }
    if let Some(value) = read_signature(profile, 36) {
        push(&mut entries, "Firma del archivo de perfil", value);
    }
    if let Some(value) = read_signature(profile, 40).map(map_vendor) {
        push(&mut entries, "Plataforma principal", value);
    }
    if let Some(value) = read_u32_be(profile, 44).map(format_profile_flags) {
        push(&mut entries, "CMM flags", value);
    }
    if let Some(value) = read_signature(profile, 48).map(map_vendor) {
        push(&mut entries, "Fabricante del dispositivo", value);
    }
    if let Some(value) = read_signature(profile, 52).map(map_model) {
        push(&mut entries, "Modelo de dispositivo", value);
    }
    if let Some(value) = read_u64_be(profile, 56).map(format_device_attributes) {
        push(&mut entries, "Atributos del dispositivo", value);
    }
    if let Some(value) = read_u32_be(profile, 64).map(format_rendering_intent) {
        push(&mut entries, "Intención de renderizado", value);
    }
    if let Some(value) = read_xyz(profile, 68) {
        push(&mut entries, "Iluminante del espacio de conexión", value);
    }
    if let Some(value) = read_signature(profile, 80).map(map_vendor) {
        push(&mut entries, "Creador de perfiles", value);
    }
    if let Some(value) = read_profile_id(profile, 84) {
        push(&mut entries, "ID de perfil", value);
    }

    let tag_table = read_tag_table(profile);

    if let Some(text) = read_text_tag(profile, &tag_table, "desc") {
        push(&mut entries, "Nombre del perfil", text);
    }
    if let Some(text) = read_text_tag(profile, &tag_table, "dmnd") {
        push(&mut entries, "Descripción del perfil", text);
    }
    if let Some(text) = read_text_tag(profile, &tag_table, "dmdd") {
        push(&mut entries, "Descripción del fabricante", text);
    }
    if let Some(text) = read_text_tag(profile, &tag_table, "cprt") {
        push(&mut entries, "Perfil derechos de autor", text);
    }
    if let Some(text) = read_text_tag(profile, &tag_table, "tech") {
        push(&mut entries, "Tecnología de dispositivo", text);
    }
    if let Some(value) = read_xyz_tag(profile, &tag_table, "wtpt") {
        push(&mut entries, "Punto blanco de los medios", value);
    }
    if let Some(value) = read_xyz_tag(profile, &tag_table, "rXYZ") {
        push(&mut entries, "Columna de matriz roja", value);
    }
    if let Some(value) = read_xyz_tag(profile, &tag_table, "gXYZ") {
        push(&mut entries, "Columna de matriz verde", value);
    }
    if let Some(value) = read_xyz_tag(profile, &tag_table, "bXYZ") {
        push(&mut entries, "Columna de matriz azul", value);
    }
    if let Some(value) = read_chad_tag(profile, &tag_table) {
        push(&mut entries, "Adaptación cromática", value);
    }
    if let Some(value) = read_curve_tag(profile, &tag_table, "rTRC") {
        push(&mut entries, "Red TRC", value);
    }
    if let Some(value) = read_curve_tag(profile, &tag_table, "gTRC") {
        push(&mut entries, "Verde TRC", value);
    }
    if let Some(value) = read_curve_tag(profile, &tag_table, "bTRC") {
        push(&mut entries, "Azul TRC", value);
    }

    entries
}

fn push(entries: &mut Vec<ReportEntry>, label: &str, value: String) {
    if value.trim().is_empty() {
        return;
    }
    entries.push(ReportEntry::info(label, value));
}

fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64_be(data: &[u8], offset: usize) -> Option<u64> {
    data.get(offset..offset + 8).map(|bytes| {
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    })
}

fn read_i32_be(data: &[u8], offset: usize) -> Option<i32> {
    read_u32_be(data, offset).map(|value| value as i32)
}

fn read_signature(data: &[u8], offset: usize) -> Option<String> {
    let slice = data.get(offset..offset + 4)?;
    let raw = String::from_utf8_lossy(slice).to_string();
    Some(raw.trim().to_string())
}

fn read_profile_version(data: &[u8]) -> Option<String> {
    let value = read_u32_be(data, 8)?;
    let major = (value >> 24) & 0xFF;
    let minor = (value >> 20) & 0x0F;
    let bugfix = (value >> 16) & 0x0F;
    Some(format!("{major}.{minor}.{bugfix}"))
}

fn read_icc_datetime(data: &[u8], offset: usize) -> Option<String> {
    let year = read_u16_be(data, offset)?;
    let month = read_u16_be(data, offset + 2)?;
    let day = read_u16_be(data, offset + 4)?;
    let hour = read_u16_be(data, offset + 6)?;
    let minute = read_u16_be(data, offset + 8)?;
    let second = read_u16_be(data, offset + 10)?;
    Some(format!(
        "{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

fn read_profile_id(data: &[u8], offset: usize) -> Option<String> {
    let slice = data.get(offset..offset + 16)?;
    if slice.iter().all(|&b| b == 0) {
        return Some("0".to_string());
    }
    Some(hex_bytes(slice))
}

fn read_xyz(data: &[u8], offset: usize) -> Option<String> {
    let x = read_i32_be(data, offset)?;
    let y = read_i32_be(data, offset + 4)?;
    let z = read_i32_be(data, offset + 8)?;
    Some(format_xyz(x, y, z))
}

fn read_tag_table(profile: &[u8]) -> HashMap<String, (usize, usize)> {
    let mut tags = HashMap::new();
    let count = match read_u32_be(profile, 128) {
        Some(count) => count as usize,
        None => return tags,
    };

    let mut offset = 132;
    for _ in 0..count {
        if offset + 12 > profile.len() {
            break;
        }
        let signature = match read_signature(profile, offset) {
            Some(sig) => sig,
            None => break,
        };
        let tag_offset = match read_u32_be(profile, offset + 4) {
            Some(value) => value as usize,
            None => break,
        };
        let tag_size = match read_u32_be(profile, offset + 8) {
            Some(value) => value as usize,
            None => break,
        };
        tags.insert(signature, (tag_offset, tag_size));
        offset += 12;
    }
    tags
}

fn read_tag_slice<'a>(
    profile: &'a [u8],
    tag_table: &HashMap<String, (usize, usize)>,
    signature: &str,
) -> Option<&'a [u8]> {
    let (offset, size) = *tag_table.get(signature)?;
    if offset + size > profile.len() || size < 4 {
        return None;
    }
    Some(&profile[offset..offset + size])
}

fn read_text_tag(
    profile: &[u8],
    tag_table: &HashMap<String, (usize, usize)>,
    signature: &str,
) -> Option<String> {
    let data = read_tag_slice(profile, tag_table, signature)?;
    parse_text_tag(data)
}

fn parse_text_tag(data: &[u8]) -> Option<String> {
    if data.len() < 8 {
        return None;
    }
    let tag_type = read_signature(data, 0)?;
    match tag_type.as_str() {
        "desc" => parse_desc_tag(data),
        "text" => parse_text_type_tag(data),
        "mluc" => parse_mluc_tag(data),
        _ => None,
    }
}

fn parse_desc_tag(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    let length = read_u32_be(data, 8)? as usize;
    let start = 12;
    let end = start + length.min(data.len().saturating_sub(start));
    let text = String::from_utf8_lossy(&data[start..end]);
    let trimmed = text.trim_end_matches('\0');
    Some(trimmed.trim().to_string())
}

fn parse_text_type_tag(data: &[u8]) -> Option<String> {
    if data.len() <= 8 {
        return None;
    }
    let text = String::from_utf8_lossy(&data[8..]);
    let trimmed = text.trim_end_matches('\0');
    Some(trimmed.trim().to_string())
}

fn parse_mluc_tag(data: &[u8]) -> Option<String> {
    if data.len() < 16 {
        return None;
    }
    let count = read_u32_be(data, 8)? as usize;
    let record_size = read_u32_be(data, 12)? as usize;
    if count == 0 || record_size < 12 {
        return None;
    }
    let record_start = 16;
    if data.len() < record_start + record_size {
        return None;
    }
    let length = read_u32_be(data, record_start + 4)? as usize;
    let offset = read_u32_be(data, record_start + 8)? as usize;
    if offset + length > data.len() {
        return None;
    }
    decode_utf16_be(&data[offset..offset + length])
}

fn decode_utf16_be(data: &[u8]) -> Option<String> {
    if data.len() % 2 != 0 {
        return None;
    }
    let mut values = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks(2) {
        values.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16(&values).ok().map(|value| value.trim().to_string())
}

fn read_xyz_tag(
    profile: &[u8],
    tag_table: &HashMap<String, (usize, usize)>,
    signature: &str,
) -> Option<String> {
    let data = read_tag_slice(profile, tag_table, signature)?;
    if data.len() < 20 {
        return None;
    }
    let tag_type = read_signature(data, 0)?;
    if tag_type != "XYZ" {
        return None;
    }
    read_xyz(data, 8)
}

fn read_chad_tag(
    profile: &[u8],
    tag_table: &HashMap<String, (usize, usize)>,
) -> Option<String> {
    let data = read_tag_slice(profile, tag_table, "chad")?;
    if data.len() < 8 + 9 * 4 {
        return None;
    }
    let tag_type = read_signature(data, 0)?;
    if tag_type != "sf32" && tag_type != "s15f" && tag_type != "s15" {
        return None;
    }
    let mut values = Vec::new();
    let mut offset = 8;
    for _ in 0..9 {
        let value = read_i32_be(data, offset)?;
        values.push(format_float(value));
        offset += 4;
    }
    Some(values.join(" "))
}

fn read_curve_tag(
    profile: &[u8],
    tag_table: &HashMap<String, (usize, usize)>,
    signature: &str,
) -> Option<String> {
    let data = read_tag_slice(profile, tag_table, signature)?;
    if data.len() < 12 {
        return None;
    }
    let tag_type = read_signature(data, 0)?;
    if tag_type == "curv" {
        let count = read_u32_be(data, 8)? as usize;
        if count == 1 && data.len() >= 14 {
            let gamma = read_u16_be(data, 12)? as f32 / 256.0;
            return Some(format!("Gamma {:.2}", gamma));
        }
    }
    Some(format!(
        "Datos binarios de {} bytes",
        data.len().saturating_sub(8)
    ))
}

fn format_xyz(x: i32, y: i32, z: i32) -> String {
    format!("{} {} {}", format_float(x), format_float(y), format_float(z))
}

fn format_float(value: i32) -> String {
    let float_value = value as f64 / 65536.0;
    if (float_value - float_value.round()).abs() < f64::EPSILON {
        format!("{}", float_value.round() as i64)
    } else {
        format!("{:.5}", float_value)
    }
}

fn hex_bytes(data: &[u8]) -> String {
    data.iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join("")
}

fn format_profile_flags(flags: u32) -> String {
    let embedded = if flags & 1 != 0 {
        "Integrado"
    } else {
        "No integrado"
    };
    let independent = if flags & 2 != 0 {
        "No independiente"
    } else {
        "Independiente"
    };
    format!("{embedded}, {independent}")
}

fn format_device_attributes(attributes: u64) -> String {
    let reflective = if attributes & 1 == 0 {
        "Reflectante"
    } else {
        "Transparente"
    };
    let glossy = if attributes & 2 == 0 {
        "Brillante"
    } else {
        "Mate"
    };
    let positive = if attributes & 4 == 0 {
        "Positivo"
    } else {
        "Negativo"
    };
    let color = if attributes & 8 == 0 {
        "Color"
    } else {
        "Blanco y negro"
    };
    format!("{reflective}, {glossy}, {positive}, {color}")
}

fn format_rendering_intent(intent: u32) -> String {
    match intent {
        0 => "Perceptivo".to_string(),
        1 => "Colorimétrico relativo".to_string(),
        2 => "Saturación".to_string(),
        3 => "Colorimétrico absoluto".to_string(),
        _ => format!("Desconocido ({intent})"),
    }
}

fn map_vendor(signature: String) -> String {
    match signature.as_str() {
        "APPL" => "Apple Computer Inc.".to_string(),
        "MSFT" => "Microsoft Corporation".to_string(),
        "SGI" => "Silicon Graphics Inc.".to_string(),
        "SUNW" => "Sun Microsystems".to_string(),
        "TGNT" => "Taligent".to_string(),
        _ => signature,
    }
}

fn map_model(signature: String) -> String {
    if signature.is_empty() {
        return "No disponible".to_string();
    }
    signature
}

fn map_device_class(signature: String) -> String {
    match signature.as_str() {
        "scnr" => "Perfil de escáner".to_string(),
        "mntr" => "Perfil del dispositivo de visualización".to_string(),
        "prtr" => "Perfil de impresora".to_string(),
        "link" => "Perfil de enlace".to_string(),
        "spac" => "Perfil del espacio de color".to_string(),
        "abst" => "Perfil abstracto".to_string(),
        "nmcl" => "Perfil de colores nominales".to_string(),
        _ => signature,
    }
}

fn map_color_space(signature: String) -> String {
    match signature.as_str() {
        "RGB" => "RGB",
        "GRAY" => "Gris",
        "CMYK" => "CMYK",
        "LAB" => "Lab",
        "XYZ" => "XYZ",
        "YCbr" => "YCbCr",
        _ => signature.as_str(),
    }
    .to_string()
}
