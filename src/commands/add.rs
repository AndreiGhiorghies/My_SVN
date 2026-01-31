use rayon::prelude::*;
use std::sync::{Arc, Mutex};

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::utils::path::RepoLocationError::*;
use crate::{
    error_data,
    utils::{
        error::ErrorData,
        hash::calculate_hash,
        index::{IndexData, get_svn_ignore, ignore_file},
        path::{
            FileInfo, copy_to_repo_objects, find_repo_root, format_path,
            get_working_directory_optimized, is_path_within,
        },
    },
};

fn add_files_parallel(
    files_to_add: &[(String, FileInfo)],
    root: &str,
    objects: &Path,
    index_data: &mut IndexData,
) -> Result<(), ErrorData> {
    let index_data = Arc::new(Mutex::new(index_data));

    let threads_count = rayon::current_num_threads();

    let chunk_size = files_to_add.len().div_ceil(threads_count);

    if chunk_size == 0 {
        return Ok(());
    }

    files_to_add.par_chunks(chunk_size).try_for_each(|chunk| {
        for (path, info) in chunk {
            let absolute_file_path = format_path(&vec![&root, &path]);
            let file_hash = calculate_hash(&absolute_file_path).map_err(|e| {
                error_data!(
                    "add_files_parallel",
                    e.to_string(),
                    "Failed to calculate hash for file"
                )
            })?;

            {
                let mut index_data = match index_data.lock() {
                    Ok(data) => data,
                    Err(e) => {
                        return Err(error_data!(
                            "add_files_parallel",
                            e.to_string(),
                            "Failed to lock index data"
                        ));
                    }
                };

                index_data.entries.insert(
                    path.clone(),
                    FileInfo {
                        hash: file_hash.clone(),
                        timestamp: info.timestamp,
                    },
                );
            }

            if !objects.join(file_hash.clone()).exists() {
                copy_to_repo_objects(&absolute_file_path, &objects.join(&file_hash)).map_err(
                    |e| {
                        error_data!(
                            "add_files_parallel",
                            e.to_string(),
                            "Failed to copy file to objects directory"
                        )
                    },
                )?;
            }
        }
        Ok::<(), ErrorData>(())
    })?;

    Ok(())
}

pub fn add(files: &Vec<String>) -> Result<(), ErrorData> {
    let root = match find_repo_root(&"./".to_string()) {
        Ok(rep_loc) => rep_loc,
        Err(e) => match e {
            ErrorData(ed) => {
                return Err(error_data!(
                    "add",
                    ed.to_string(),
                    "Failed to find repository root"
                ));
            }
            RepositoryNotFoundError => {
                println!("fatal: not a svn repository (or any of the parent directories): .my_svn");
                return Ok(());
            }
        },
    };

    let objects = PathBuf::from(format_path(&vec![&root.root, &".my_svn", &"objects"]));

    let mut index_data = match IndexData::new() {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "add",
                e.to_string(),
                "Failed to load index data"
            ));
        }
    };

    let start_path = if root.relative.is_empty() {
        String::new()
    } else {
        format_path(&vec![&root.relative, &""])
    };

    let ignore_rules = get_svn_ignore(&format_path(&vec![&root.root, ".svnignore"]));

    if files[0] == "." {
        let folder_files = match get_working_directory_optimized(&root.root) {
            Ok(f) => f,
            Err(e) => {
                return Err(error_data!(
                    "add",
                    e.to_string(),
                    "Failed to get working directory files"
                ));
            }
        };

        let files_to_add = folder_files
            .entries
            .iter()
            .filter(|(path, info)| {
                !ignore_file(path, &ignore_rules)
                    && path.starts_with(&start_path)
                    && (!index_data.entries.contains_key(*path)
                        || index_data.entries[*path].timestamp != info.timestamp)
            })
            .map(|(path, info)| (path.clone(), info.clone()))
            .collect::<Vec<(String, FileInfo)>>();

        add_files_parallel(&files_to_add, &root.root, &objects, &mut index_data)
            .map_err(|e| error_data!("add", e.to_string(), "Failed to add files in parallel"))?;

        let keys_to_remove: Vec<String> = index_data
            .entries
            .keys()
            .filter(|path| !folder_files.entries.contains_key(*path))
            .cloned()
            .collect();

        for path in keys_to_remove {
            index_data.entries.remove(&path);
        }
    } else {
        let mut files_to_add: Vec<(String, FileInfo)> = Vec::new();

        for p in files {
            if !Path::new(p).exists() {
                if index_data.entries.contains_key(p) {
                    index_data.entries.remove(p);
                    continue;
                } else {
                    let mut is_valid_folder = false;
                    let path = if p.ends_with("/") || p.ends_with('\\') {
                        &p[..p.len() - 1]
                    } else {
                        p
                    };
                    let relative_folder_path =
                        format_path(&vec![&format!("{}{}", start_path, path), ""]);

                    for (key, _) in index_data.entries.clone() {
                        if key.starts_with(&relative_folder_path) {
                            index_data.entries.remove(&key);
                            is_valid_folder = true;
                        }
                    }

                    if is_valid_folder {
                        continue;
                    }
                }

                println!("pathspec '{}' did not match any files", p);
                return Ok(());
            }

            let absolute_path = match dunce::canonicalize(p) {
                Ok(pa) => pa.to_string_lossy().to_string(),
                Err(e) => {
                    return Err(error_data!(
                        "add",
                        e.to_string(),
                        "Failed to canonicalize path"
                    ));
                }
            };

            if !is_path_within(&root.root, &absolute_path) {
                println!("Error: '{}' is outside repository at '{}'", p, root.root);
                return Ok(());
            }

            let relative_path = if root.root == absolute_path {
                String::from("")
            } else {
                match absolute_path.strip_prefix(&format_path(&vec![&root.root, ""])) {
                    Some(rel_path) => rel_path.to_string(),
                    None => {
                        return Err(error_data!(
                            "add",
                            "".to_string(),
                            "Failed to get relative path"
                        ));
                    }
                }
            };

            if Path::new(&absolute_path).is_file() {
                if ignore_file(&relative_path, &ignore_rules) {
                    continue;
                }

                let timestamp = fs::metadata(&absolute_path)
                    .map_err(|e| error_data!("add", e.to_string(), "Failed to get file metadata"))?
                    .modified()
                    .map_err(|e| {
                        error_data!("add", e.to_string(), "Failed to get file modified time")
                    })?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| {
                        error_data!(
                            "add",
                            e.to_string(),
                            "Failed to convert file modified time to UNIX timestamp"
                        )
                    })?
                    .as_secs();

                if !index_data.entries.contains_key(&relative_path)
                    || index_data.entries[&relative_path].timestamp != timestamp
                {
                    files_to_add.push((
                        relative_path.clone(),
                        FileInfo {
                            hash: String::new(),
                            timestamp,
                        },
                    ));
                }
            } else if Path::new(&absolute_path).is_dir() {
                let mut folder_files = match get_working_directory_optimized(&absolute_path) {
                    Ok(f) => f,
                    Err(e) => {
                        return Err(error_data!(
                            "add",
                            e.to_string(),
                            "Failed to get working directory files"
                        ));
                    }
                };

                for (path, info) in &mut folder_files.entries {
                    let absolute_file_path = format_path(&vec![&root.root, &absolute_path, &path]);
                    let relative_file_path = match absolute_file_path
                        .strip_prefix(&format_path(&vec![&root.root, ""]))
                    {
                        Some(rel_path) => rel_path.to_string(),
                        None => {
                            return Err(error_data!(
                                "add",
                                "".to_string(),
                                "Failed to canonicalize path"
                            ));
                        }
                    };

                    if ignore_file(&relative_file_path, &ignore_rules) {
                        continue;
                    }

                    if !index_data.entries.contains_key(&relative_file_path)
                        || index_data.entries[&relative_file_path].timestamp != info.timestamp
                    {
                        files_to_add.push((relative_file_path.clone(), info.clone()));
                    }
                }
            }
        }

        add_files_parallel(&files_to_add, &root.root, &objects, &mut index_data)
            .map_err(|e| error_data!("add", e.to_string(), "Failed to add files in parallel"))?;
    }

    index_data
        .save_index()
        .map_err(|e| error_data!("add", e.to_string(), "Failed to save index data"))?;

    Ok(())
}
