use std::{collections::HashMap, fs};

use crate::{
    error_data,
    utils::error::ErrorData,
    utils::path::{FileInfo, format_path},
};

use crate::utils::json::load_json;
use crate::utils::path::RepoLocationError::*;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct IndexData {
    absolute_path: String,
    pub entries: HashMap<String, FileInfo>,
}

impl IndexData {
    pub fn new() -> Result<Self, ErrorData> {
        let repo_location = match crate::utils::path::find_repo_root(&String::from("./")) {
            Ok(loc) => loc,
            Err(e) => {
                return Err(error_data!(
                    "IndexData::new",
                    match e {
                        ErrorData(ed) => ed.to_string(),
                        RepositoryNotFoundError => String::new(),
                    },
                    "Failed to find repository root"
                ));
            }
        };

        let absolute_path = format_path(&vec![&repo_location.root, ".my_svn", "index"]);

        let entries: HashMap<String, FileInfo> = match load_json(&absolute_path) {
            Ok(ent) => ent,
            Err(e) => {
                return Err(error_data!(
                    "IndexData::new",
                    e.to_string(),
                    "Failed to load index JSON data"
                ));
            }
        };

        Ok(Self {
            absolute_path,
            entries,
        })
    }

    pub fn save_index(self) -> Result<(), ErrorData> {
        let json: String = match serde_json::to_string_pretty(&self.entries) {
            Ok(j) => j,
            Err(e) => {
                return Err(error_data!(
                    "IndexData::save_index",
                    e.to_string(),
                    "Failed to serialize index entries to JSON"
                ));
            }
        };

        std::fs::write(self.absolute_path, json).map_err(|e| {
            error_data!(
                "IndexData::save_index",
                e.to_string(),
                "Failed to write index data to file"
            )
        })?;

        Ok(())
    }
}

pub fn get_svn_ignore(address: &str) -> Vec<String> {
    let content = match fs::read_to_string(address) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content.lines().map(|s| s.to_string()).collect()
}

pub fn ignore_file(path: &String, rules: &Vec<String>) -> bool {
    if path.starts_with(&format_path(&vec![".my_svn", ""])) {
        return true;
    }

    for i in rules {
        if path == i {
            return true;
        }

        if i.ends_with('/') && path.starts_with(&format_path(&vec![&i[..i.len() - 1], ""])) {
            return true;
        }

        if i.starts_with('*') && path.ends_with(&i[1..]) {
            return true;
        }
    }

    false
}
