use colored::Colorize;
use std::{collections::HashMap};

use crate::{
    commands::{branch::get_current_branch, commit::read_commit},
    error_data,
    utils::{
        error::ErrorData,
        hash::calculate_hash,
        index::{IndexData, get_svn_ignore, ignore_file},
        path::{
            FileInfo, RepoLocation, RepoLocationError::*, find_repo_root, format_path,
            get_working_directory_optimized, relative_to_root,
        },
    },
};

fn compare_last_commit_with_index(
    index_files: &HashMap<String, FileInfo>,
    commit_data: &mut HashMap<String, FileInfo>,
    root: &RepoLocation,
) -> bool {
    let mut first = false;

    for i in index_files {
        if let Some(commit_hash) = commit_data.get(i.0) {
            if i.1.hash != commit_hash.hash {
                if !first {
                    println!("Changes to be committed:");
                    first = true;
                }
                println!(
                    "{}",
                    format!(
                        "        modified:   {}",
                        relative_to_root(
                            &format_path(&vec![&root.root, &root.relative]),
                            &format_path(&vec![&root.root, &i.0])
                        )
                    )
                    .green()
                );
            }
            commit_data.remove(i.0);
        } else {
            if !first {
                println!("Changes to be committed:");
                first = true;
            }
            println!(
                "{}",
                format!(
                    "        new file:   {}",
                    relative_to_root(
                        &format_path(&vec![&root.root, &root.relative]),
                        &format_path(&vec![&root.root, &i.0])
                    )
                )
                .green()
            );
        }
    }

    for i in commit_data {
        if !first {
            println!("Changes to be committed:");
            first = true;
        }
        println!(
            "{}",
            format!(
                "        deleted:    {}",
                relative_to_root(
                    &format_path(&vec![&root.root, &root.relative]),
                    &format_path(&vec![&root.root, &i.0])
                )
            )
            .green()
        );
    }

    first
}

fn compare_index_with_working_directory(
    index_files: &mut HashMap<String, FileInfo>,
    root: &RepoLocation,
) -> Result<bool, ErrorData> {
    let mut changes = false;

    let working_directory = match get_working_directory_optimized(&root.root) {
        Ok(f) => f,
        Err(e) => {
            return Err(error_data!(
                "status",
                e.to_string(),
                "Failed to get working directory files"
            ));
        }
    };

    let ignore_rules = get_svn_ignore(&format_path(&vec![&root.root, ".svnignore"]));

    let mut untracked_files: Vec<String> = Vec::new();
    let mut modified_files: Vec<String> = Vec::new();

    for (path, info) in working_directory.entries {
        if ignore_file(&path, &ignore_rules) {
            continue;
        }

        if index_files.contains_key(&path) {
            if index_files[&path].timestamp != info.timestamp
                && index_files[&path].hash
                    != calculate_hash(&format_path(&vec![&root.root, &path])).map_err(|e| {
                        error_data!("status", e.to_string(), "Failed to calculate file hash")
                    })?
            {
                modified_files.push(path.to_owned());
            }

            index_files.remove(&path);
        } else {
            untracked_files.push(path);
        }
    }

    if !modified_files.is_empty() || !index_files.is_empty() {
        changes = true;
        println!("Changes not staged for commit:");
    }

    for i in modified_files {
        println!(
            "{}",
            format!(
                "        modified:   {}",
                relative_to_root(
                    &format_path(&vec![&root.root, &root.relative]),
                    &format_path(&vec![&root.root, &i])
                )
            )
            .red()
        );
    }

    for i in index_files {
        println!(
            "{}",
            format!(
                "        deleted:    {}",
                relative_to_root(
                    &format_path(&vec![&root.root, &root.relative]),
                    &format_path(&vec![&root.root, &i.0])
                )
            )
            .red()
        );
    }

    if !untracked_files.is_empty() {
        changes = true;
        println!("Untracked files:");
    }
    for i in untracked_files {
        println!(
            "        {}",
            relative_to_root(
                &format_path(&vec![&root.root, &root.relative]),
                &format_path(&vec![&root.root, &i])
            )
            .red()
        );
    }

    Ok(changes)
}

pub fn status() -> Result<(), ErrorData> {
    let root = match find_repo_root(&"./".to_string()) {
        Ok(rep_loc) => rep_loc,
        Err(e) => match e {
            ErrorData(ed) => {
                return Err(error_data!(
                    "status",
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

    let mut index_files = match IndexData::new() {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "status",
                e.to_string(),
                "Failed to load index data"
            ));
        }
    };

    let branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "status",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };

    let mut commit_data = match read_commit(&root.root, &branch) {
        Ok(cd) => cd,
        Err(e) => {
            return Err(error_data!(
                "status",
                e.to_string(),
                "Failed to read last commit data"
            ));
        }
    };

    let mut changes = compare_last_commit_with_index(&index_files.entries, &mut commit_data, &root);
    changes |= match compare_index_with_working_directory(&mut index_files.entries, &root) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "status",
                e.to_string(),
                "Failed to compare index with working directory"
            ));
        }
    };

    if !changes {
        println!("Nothing to commit, working tree clean");
    }

    Ok(())
}
