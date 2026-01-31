use std::collections::HashSet;
use std::path::Path;
use std::{fs, vec};

use crate::commands::branch::get_current_branch;
use crate::commands::commit::read_commit;
use crate::error_data;
use crate::utils::hash::calculate_hash;
use crate::utils::index::IndexData;
use crate::utils::path::{
    FileInfo, RepoLocationError::*, format_path, get_working_directory_optimized,
};
use crate::utils::{error::ErrorData, path::find_repo_root};

pub fn checkout(branch_name: &str) -> Result<(), ErrorData> {
    let root = match find_repo_root(&"./".to_string()) {
        Ok(rep_loc) => rep_loc,
        Err(e) => match e {
            ErrorData(ed) => {
                return Err(error_data!(
                    "checkout",
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

    if !Path::new(&format_path(&vec![
        &root.root,
        ".my_svn",
        "refs",
        "heads",
        &branch_name,
    ]))
    .exists()
    {
        println!("fatal: A branch named '{}' does not exist.", branch_name);
        return Ok(());
    }

    let current_branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "checkout",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };

    if current_branch == branch_name {
        println!("You are already on branch '{}'.", branch_name);
        return Ok(());
    }

    let mut current_commit = match read_commit(&root.root, &current_branch) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "checkout",
                e.to_string(),
                "Failed to read current branch commit"
            ));
        }
    };
    let checkout_commit = match read_commit(&root.root, branch_name) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "checkout",
                e.to_string(),
                "Failed to read checkout branch commit"
            ));
        }
    };

    let mut working_dir_files = match get_working_directory_optimized(&root.root) {
        Ok(wd) => wd,
        Err(e) => {
            return Err(error_data!(
                "checkout",
                e.to_string(),
                "Failed to get working directory files"
            ));
        }
    };

    let mut same_files: HashSet<String> = HashSet::new();

    for (path, info) in checkout_commit.iter() {
        if current_commit.contains_key(path) {
            if current_commit[path].hash == info.hash {
                same_files.insert(path.clone());
            } else if working_dir_files.entries.contains_key(path) {
                if working_dir_files.entries[path].timestamp == info.timestamp {
                    same_files.insert(path.clone());
                } else {
                    if current_commit[path].timestamp == working_dir_files.entries[path].timestamp {
                        working_dir_files
                            .entries
                            .insert(path.clone(), current_commit[path].clone());
                    } else {
                        let file_hash = match calculate_hash(&format_path(&vec![&root.root, &path]))
                        {
                            Ok(h) => h,
                            Err(e) => {
                                return Err(error_data!(
                                    "checkout",
                                    e.to_string(),
                                    "Failed to calculate file hash"
                                ));
                            }
                        };

                        working_dir_files.entries.insert(
                            path.clone(),
                            FileInfo {
                                hash: file_hash,
                                timestamp: working_dir_files.entries[path].timestamp,
                            },
                        );
                    }

                    if working_dir_files.entries[path].hash == info.hash {
                        same_files.insert(path.clone());
                    } else if working_dir_files.entries[path].hash != current_commit[path].hash {
                        println!("File {} has uncommitted changes. Cannot checkout!", path);

                        return Ok(());
                    }
                }
            } else {
                println!("File {} has uncommitted changes. Cannot checkout!", path);

                return Ok(());
            }

            current_commit.remove(path);
        } else if working_dir_files.entries.contains_key(path) {
            println!("File {} has uncommitted changes. Cannot checkout!", path);

            return Ok(());
        }
    }

    for path in current_commit.keys() {
        if working_dir_files.entries.contains_key(path) {
            if working_dir_files.entries[path].hash.is_empty() {
                if working_dir_files.entries[path].timestamp == current_commit[path].timestamp {
                    continue;
                } else if checkout_commit.contains_key(path)
                    && working_dir_files.entries[path].timestamp == checkout_commit[path].timestamp
                {
                    working_dir_files
                        .entries
                        .insert(path.clone(), checkout_commit[path].clone());
                } else {
                    let file_hash = match calculate_hash(&format_path(&vec![&root.root, &path])) {
                        Ok(h) => h,
                        Err(e) => {
                            return Err(error_data!(
                                "checkout",
                                e.to_string(),
                                "Failed to calculate file hash during checkout"
                            ));
                        }
                    };

                    working_dir_files.entries.insert(
                        path.clone(),
                        FileInfo {
                            hash: file_hash,
                            timestamp: working_dir_files.entries[path].timestamp,
                        },
                    );
                }
            }

            if working_dir_files.entries[path].hash != current_commit[path].hash {
                println!("File {} has uncommitted changes. Cannot checkout!", path);
                return Ok(());
            }
        }
    }

    for path in current_commit.keys() {
        if working_dir_files.entries.contains_key(path) {
            fs::remove_file(format_path(&vec![&root.root, &path])).map_err(|e| {
                error_data!(
                    "checkout",
                    e.to_string(),
                    "Failed to remove file during checkout"
                )
            })?;
        }
    }

    for (path, info) in checkout_commit.iter() {
        if !same_files.contains(path) {
            let object_path = format_path(&vec![&root.root, ".my_svn", "objects", &info.hash]);
            let dest_path = format_path(&vec![&root.root, &path]);

            if let Some(parent) = Path::new(&dest_path).parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    error_data!(
                        "checkout",
                        e.to_string(),
                        "Failed to create parent directories during checkout"
                    )
                })?;
            }

            fs::copy(object_path, dest_path).map_err(|e| {
                error_data!(
                    "checkout",
                    e.to_string(),
                    "Failed to copy file during checkout"
                )
            })?;
        }
    }

    let head_path = format_path(&vec![&root.root, ".my_svn", "HEAD"]);
    fs::write(head_path, branch_name).map_err(|e| {
        error_data!(
            "checkout",
            e.to_string(),
            "Failed to update HEAD during checkout"
        )
    })?;

    let mut index_files: IndexData = IndexData::new().map_err(|e| {
        error_data!(
            "checkout",
            e.to_string(),
            "Failed to load index data during checkout"
        )
    })?;

    index_files.entries.clear();
    index_files.entries = checkout_commit;

    index_files.save_index().map_err(|e| {
        error_data!(
            "checkout",
            e.to_string(),
            "Failed to save index data during checkout"
        )
    })?;

    Ok(())
}
