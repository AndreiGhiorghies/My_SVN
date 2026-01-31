use serde::de::DeserializeOwned;
use std::{fs::File, io::Read};

use crate::{error_data, utils::error::ErrorData};

pub fn load_json<T: DeserializeOwned>(path: &String) -> Result<T, ErrorData> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            return Err(error_data!(
                "load_json",
                e.to_string(),
                "Failed to open JSON file"
            ));
        }
    };

    let mut data = String::new();
    file.read_to_string(&mut data)
        .map_err(|e| error_data!("load_json", e.to_string(), "Failed to read JSON file"))?;

    let value = match serde_json::from_str::<T>(&data) {
        Ok(v) => v,
        Err(e) => {
            return Err(error_data!(
                "load_json",
                e.to_string(),
                "Failed to parse JSON file"
            ));
        }
    };

    Ok(value)
}
