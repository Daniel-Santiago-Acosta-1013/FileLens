//! Cálculo de hashes para detectar cambios en archivos pequeños.

use md5::Md5;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

const HASH_SIZE_LIMIT: u64 = 32 * 1024 * 1024; // 32 MiB

#[derive(Clone, Debug)]
pub struct HashSummary {
    pub md5: String,
    pub sha256: String,
}

/// Devuelve los hashes del archivo o un mensaje cuando no aplica.
pub fn file_hashes(path: &Path, metadata: &Metadata) -> HashSummary {
    if !metadata.is_file() {
        return HashSummary {
            md5: "No aplica".to_string(),
            sha256: "No aplica".to_string(),
        };
    }

    if metadata.len() > HASH_SIZE_LIMIT {
        let value = format!("Omitido (> {} MiB)", HASH_SIZE_LIMIT / (1024 * 1024));
        return HashSummary {
            md5: value.clone(),
            sha256: value,
        };
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            let value = format!("No disponible ({error})");
            return HashSummary {
                md5: value.clone(),
                sha256: value,
            };
        }
    };

    let mut md5 = Md5::new();
    let mut sha256 = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                md5.update(&buffer[..bytes_read]);
                sha256.update(&buffer[..bytes_read]);
            }
            Err(error) => {
                let value = format!("No disponible ({error})");
                return HashSummary {
                    md5: value.clone(),
                    sha256: value,
                };
            }
        }
    }

    let md5_digest = md5.finalize();
    let sha_digest = sha256.finalize();
    HashSummary {
        md5: format!("{:x}", md5_digest),
        sha256: format!("{:x}", sha_digest),
    }
}

/// Devuelve el hash SHA-256 del archivo o un mensaje cuando no aplica.
#[allow(dead_code)]
pub fn file_hash(path: &Path, metadata: &Metadata) -> String {
    file_hashes(path, metadata).sha256
}
