use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::commands::branch::{get_branch_hash, get_current_branch};
use crate::commands::commit::{commit, find_base_commit, read_commit, read_commit_from_hash};
use crate::error_data;
use crate::utils::hash::calculate_hash;
use crate::utils::index::IndexData;
use crate::utils::path::{
    FileInfo, RepoLocationError::*, format_path, get_working_directory_optimized,
};
use crate::utils::{error::ErrorData, path::find_repo_root};

pub fn merge(branch_name: &str) -> Result<(), ErrorData> {
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

    let branch_path = format_path(&vec![&root.root, ".my_svn", "refs", "heads", &branch_name]);
    if !Path::new(&branch_path).exists() {
        println!("fatal: A branch named '{}' does not exist.", branch_name);
        return Ok(());
    }

    let current_branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };

    if current_branch == branch_name {
        println!("fatal: You are already on branch '{}'.", branch_name);
        return Ok(());
    }

    let mut your_commit = match read_commit(&root.root, &current_branch) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to read current branch commit"
            ));
        }
    };

    let target_commit = match read_commit(&root.root, branch_name) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to read target branch commit"
            ));
        }
    };

    let current_commit_hash = match get_branch_hash(&root.root, &current_branch) {
        Ok(h) => h,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to get current branch hash"
            ));
        }
    };
    let target_commit_hash = match get_branch_hash(&root.root, branch_name) {
        Ok(h) => h,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to get target branch hash"
            ));
        }
    };

    let base_commit_hash =
        match find_base_commit(&current_commit_hash, &target_commit_hash, &root.root) {
            Ok(data) => match data {
                Some(bc) => bc,
                None => {
                    println!("fatal: Could not find a common base commit for the merge.");
                    return Ok(());
                }
            },
            Err(e) => {
                return Err(error_data!(
                    "merge",
                    e.to_string(),
                    "Failed to find base commit"
                ));
            }
        };

    let base_commit = match read_commit_from_hash(&root.root, &base_commit_hash) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to read base commit"
            ));
        }
    };

    let mut working_dir_files = match get_working_directory_optimized(&root.root) {
        Ok(wd) => wd,
        Err(e) => {
            return Err(error_data!(
                "merge",
                e.to_string(),
                "Failed to get working directory files"
            ));
        }
    };

    let mut dont_copy: HashSet<String> = HashSet::new();

    for (path, info) in target_commit.iter() {
        if working_dir_files.entries.contains_key(path) {
            if working_dir_files.entries[path].timestamp == info.timestamp {
                working_dir_files.entries.insert(path.clone(), info.clone());
            } else if your_commit.contains_key(path)
                && your_commit[path].timestamp == working_dir_files.entries[path].timestamp
            {
                working_dir_files
                    .entries
                    .insert(path.clone(), your_commit[path].clone());
            } else {
                let file_hash = match calculate_hash(&format_path(&vec![&root.root, &path])) {
                    Ok(h) => h,
                    Err(e) => {
                        return Err(error_data!(
                            "merge",
                            e.to_string(),
                            "Failed to calculate file hash during merge"
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

                if your_commit.contains_key(path)
                    && your_commit[path].hash == working_dir_files.entries[path].hash
                {
                    your_commit.insert(path.clone(), working_dir_files.entries[path].clone());
                }
            }
        }

        if your_commit.contains_key(path) {
            if working_dir_files.entries.contains_key(path)
                && working_dir_files.entries[path].hash != info.hash
                && working_dir_files.entries[path].hash != your_commit[path].hash
            {
                println!("fatal: Uncommitted changes in file {}!", path);
                return Ok(());
            }

            let mut same_file = true;

            if base_commit.contains_key(path) {
                if base_commit[path].hash != your_commit[path].hash
                    && your_commit[path].hash != info.hash
                    && base_commit[path].hash != info.hash
                {
                    println!("fatal: Conflict at file {}!", path);
                    return Ok(());
                } else if base_commit[path].hash == your_commit[path].hash
                    && your_commit[path].hash != info.hash
                {
                    same_file = false;
                    your_commit.insert(path.clone(), info.clone());
                }
            } else if your_commit[path].hash != info.hash {
                println!("fatal: Conflict at file {}!", path);
                return Ok(());
            }

            if same_file {
                dont_copy.insert(path.clone());
            }
        } else if working_dir_files.entries.contains_key(path)
            && working_dir_files.entries[path].hash != info.hash
        {
            println!("fatal: Uncommitted changes in file {}!", path);
            return Ok(());
        } else if !base_commit.contains_key(path) {
            your_commit.insert(path.clone(), info.clone());
        } else if base_commit[path].hash != info.hash {
            println!("fatal: Conflict at file {}!", path);
            return Ok(());
        }
    }

    let mut to_delete: Vec<String> = Vec::new();

    for (path, info) in your_commit.iter() {
        if !target_commit.contains_key(path) {
            if base_commit.contains_key(path) {
                if base_commit[path].hash != info.hash {
                    println!("fatal: Conflict at file {}!", path);
                    return Ok(());
                } else {
                    to_delete.push(path.clone());
                }
            } else {
                dont_copy.insert(path.clone());
            }
        }
    }

    for path in &to_delete {
        your_commit.remove(path);
    }

    for (path, info) in your_commit.iter() {
        if dont_copy.contains(path) {
            continue;
        }

        let object_path = format_path(&vec![&root.root, ".my_svn", "objects", &info.hash]);
        let dest_path = format_path(&vec![&root.root, &path]);

        if let Some(parent) = Path::new(&dest_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                error_data!(
                    "merge",
                    e.to_string(),
                    "Failed to create parent directories during merge"
                )
            })?;
        }

        fs::copy(object_path, dest_path)
            .map_err(|e| error_data!("merge", e.to_string(), "Failed to copy file during merge"))?;
    }

    for path in to_delete {
        fs::remove_file(root.root.clone() + "\\" + &path).map_err(|e| {
            error_data!("merge", e.to_string(), "Failed to remove file during merge")
        })?;
    }

    let mut index_files: IndexData = IndexData::new().map_err(|e| {
        error_data!(
            "merge",
            e.to_string(),
            "Failed to load index data during merge"
        )
    })?;

    index_files.entries.clear();

    for (path, info) in your_commit.iter() {
        index_files.entries.insert(path.clone(), info.clone());
    }

    index_files.save_index().map_err(|e| {
        error_data!(
            "merge",
            e.to_string(),
            "Failed to save index data during merge"
        )
    })?;

    commit(format!("Merge branch {}", branch_name), &target_commit_hash)
        .map_err(|e| error_data!("merge", e.to_string(), "Failed to create merge commit"))?;

    Ok(())
}
