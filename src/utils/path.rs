use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use pathdiff::diff_paths;
use walkdir::WalkDir;

use crate::{error_data, utils::error::ErrorData};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct FileInfo {
    pub hash: String,
    pub timestamp: u64,
}

pub struct WorkingDirectoryFiles {
    pub entries: HashMap<String, FileInfo>,
}

pub struct RepoLocation {
    pub root: String,
    pub relative: String,
}

pub enum RepoLocationError {
    ErrorData(ErrorData),
    RepositoryNotFoundError,
}

pub fn get_absolute_path() -> Result<String, ErrorData> {
    match dunce::canonicalize("./") {
        Ok(path_buf) => {
            let path_buf_str = match path_buf.to_str() {
                Some(p) => p,
                None => {
                    return Err(error_data!(
                        "get_absolute_path",
                        String::from("Path contains invalid UTF-8"),
                        "Failed to convert path to string"
                    ));
                }
            };

            Ok(format_path(&vec![path_buf_str, ".my_svn", ""]))
        }
        Err(e) => Err(error_data!(
            "get_absolute_path",
            e.to_string(),
            "Failed to canonicalize path"
        )),
    }
}

pub fn format_path(pahts: &Vec<&str>) -> String {
    let mut path_buff = PathBuf::new();
    for paht in pahts {
        path_buff = path_buff.join(paht);
    }

    path_buff.to_string_lossy().to_string()
}

pub fn find_repo_root(start_path: &String) -> Result<RepoLocation, RepoLocationError> {
    let start = match dunce::canonicalize(start_path) {
        Ok(p) => p,
        Err(e) => {
            return Err(RepoLocationError::ErrorData(error_data!(
                "find_repo_root",
                e.to_string(),
                "Failed to canonicalize path"
            )));
        }
    };

    let mut dir = start.clone();

    loop {
        if dir.join(".my_svn").exists() {
            let relative = match start.strip_prefix(&dir) {
                Ok(r) => r.to_path_buf(),
                Err(e) => {
                    return Err(RepoLocationError::ErrorData(error_data!(
                        "find_repo_root",
                        e.to_string(),
                        "Failed to get relative path"
                    )));
                }
            };

            return Ok(RepoLocation {
                root: dir.to_string_lossy().to_string(),
                relative: relative.to_string_lossy().to_string(),
            });
        }

        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            return Err(RepoLocationError::RepositoryNotFoundError);
        }
    }
}

pub fn relative_to_root(root: &String, path: &String) -> String {
    let root_path = Path::new(root);
    let file_path = Path::new(path);

    match diff_paths(file_path, root_path) {
        Some(p) => p.to_string_lossy().to_string(),
        None => path.to_string(),
    }
}

/* pub fn get_working_directory(start_path: &str) -> Result<WorkingDirectoryFiles, ErrorData> {
    let mut files = WorkingDirectoryFiles {
        entries: HashMap::new(),
    };

    for entry in WalkDir::new(start_path).into_iter().filter_map(|s| s.ok()) {
        let path = entry.path().to_string_lossy().to_string();

        let mut relative_path = match path.strip_prefix(start_path) {
            Some(p) => p.to_string(),
            None => path.clone(),
        };

        if relative_path.starts_with(std::path::MAIN_SEPARATOR) {
            relative_path = relative_path[1..].to_string();
        }

        if relative_path.starts_with(".my_svn") {
            continue;
        }

        if entry.path().is_file() {
            let file_hash = match calculate_hash(&path) {
                Ok(h) => h,
                Err(e) => {
                    return Err(error_data!(
                        "get_working_directory",
                        e.to_string(),
                        "Failed to calculate file hash"
                    ));
                }
            };

            let timestamp = entry.metadata().map_err(|e| error_data!(
                "get_working_directory",
                e.to_string(),
                "Failed to get file metadata"
            ))?.modified().map_err(|e| error_data!(
                "get_working_directory",
                e.to_string(),
                "Failed to get file modified time"
            ))?.duration_since(std::time::UNIX_EPOCH).map_err(|e| error_data!(
                "get_working_directory",
                e.to_string(),
                "Failed to convert file modified time to UNIX timestamp"
            ))?.as_secs();

            files.entries.insert(
                relative_path,
                FileInfo { hash: file_hash, timestamp },
            );
        }
    }

    Ok(files)
} */

pub fn get_working_directory_optimized(
    start_path: &str,
) -> Result<WorkingDirectoryFiles, ErrorData> {
    let mut files = WorkingDirectoryFiles {
        entries: HashMap::new(),
    };

    for entry in WalkDir::new(start_path).into_iter().filter_map(|s| s.ok()) {
        let path = entry.path().to_string_lossy().to_string();

        let mut relative_path = match path.strip_prefix(start_path) {
            Some(p) => p.to_string(),
            None => path.clone(),
        };

        if relative_path.starts_with(std::path::MAIN_SEPARATOR) {
            relative_path = relative_path[1..].to_string();
        }

        if relative_path.starts_with(".my_svn") {
            continue;
        }

        if entry.path().is_file() {
            let timestamp = entry
                .metadata()
                .map_err(|e| {
                    error_data!(
                        "get_working_directory",
                        e.to_string(),
                        "Failed to get file metadata"
                    )
                })?
                .modified()
                .map_err(|e| {
                    error_data!(
                        "get_working_directory",
                        e.to_string(),
                        "Failed to get file modified time"
                    )
                })?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| {
                    error_data!(
                        "get_working_directory",
                        e.to_string(),
                        "Failed to convert file modified time to UNIX timestamp"
                    )
                })?
                .as_secs();

            files.entries.insert(
                relative_path,
                FileInfo {
                    hash: String::new(),
                    timestamp,
                },
            );
        }
    }

    Ok(files)
}

pub fn is_path_within(base: &str, target: &str) -> bool {
    let base_path = Path::new(base);
    let target_path = Path::new(target);

    target_path.starts_with(base_path)
}

pub fn copy_to_repo_objects(src: &str, dest: &PathBuf) -> Result<(), ErrorData> {
    std::fs::copy(src, dest).map_err(|e| {
        error_data!(
            "copy_to_repo_objects",
            e.to_string(),
            "Failed to copy file to repository objects"
        )
    })?;

    Ok(())
}
