//! Cálculo de hashes para detectar cambios en archivos pequeños.

use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

const HASH_SIZE_LIMIT: u64 = 32 * 1024 * 1024; // 32 MiB

/// Devuelve el hash SHA-256 del archivo o un mensaje cuando no aplica.
pub fn file_hash(path: &Path, metadata: &Metadata) -> String {
    if !metadata.is_file() {
        return "No aplica".to_string();
    }

    if metadata.len() > HASH_SIZE_LIMIT {
        return format!("Omitido (> {} MiB)", HASH_SIZE_LIMIT / (1024 * 1024));
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return format!("No disponible ({error})"),
    };

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => hasher.update(&buffer[..bytes_read]),
            Err(error) => return format!("No disponible ({error})"),
        }
    }

    let digest = hasher.finalize();
    format!("{:x}", digest)
}
