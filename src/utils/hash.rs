use sha1::{Digest, Sha1};
use std::{fs, io::Read};

use crate::{error_data, utils::error::ErrorData};

pub fn calculate_hash(path: &String) -> Result<String, ErrorData> {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return Err(error_data!(
                "calculate_hash",
                e.to_string(),
                "Failed to open file for hashing"
            ));
        }
    };

    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 4096];

    loop {
        let read_bytes = match file.read(&mut buffer) {
            Ok(b) => b,
            Err(e) => {
                return Err(error_data!(
                    "calculate_hash",
                    e.to_string(),
                    "Failed to read file for hashing"
                ));
            }
        };

        if read_bytes == 0 {
            break;
        }

        hasher.update(&buffer[..read_bytes]);
    }

    Ok(hex::encode(hasher.finalize()))
}
