use chrono::{DateTime, Local};
use colored::Colorize;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

use crate::commands::branch::{get_branch_hash, get_current_branch};
use crate::commands::commit::CommitObject;
use crate::error_data;
use crate::utils::json::load_json;
use crate::utils::path::{RepoLocationError::*, format_path};
use crate::utils::{error::ErrorData, path::find_repo_root};

struct CommitDataForLog {
    commit: CommitObject,
    hash: String,
}

impl PartialEq for CommitDataForLog {
    fn eq(&self, other: &Self) -> bool {
        self.commit.timestamp == other.commit.timestamp && self.hash == other.hash
    }
}

impl Eq for CommitDataForLog {}

impl Ord for CommitDataForLog {
    fn cmp(&self, other: &Self) -> Ordering {
        self.commit
            .timestamp
            .cmp(&other.commit.timestamp)
            .then_with(|| self.hash.cmp(&other.hash))
    }
}

impl PartialOrd for CommitDataForLog {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn log() -> Result<(), ErrorData> {
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

    let current_branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "log",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };

    let mut commit_hash = match get_branch_hash(&root.root, &current_branch) {
        Ok(h) => h,
        Err(e) => {
            return Err(error_data!(
                "log",
                e.to_string(),
                "Failed to get current branch hash"
            ));
        }
    };

    let mut heap: BinaryHeap<CommitDataForLog> = BinaryHeap::new();
    let mut commit_fr: HashSet<(u64, String)> = HashSet::new();

    heap.push(CommitDataForLog {
        commit: match load_json(&format_path(&vec![
            &root.root,
            ".my_svn",
            "objects",
            &commit_hash,
        ])) {
            Ok(c) => c,
            Err(e) => {
                return Err(error_data!(
                    "log",
                    e.to_string(),
                    "Failed to load commit object"
                ));
            }
        },
        hash: commit_hash.clone(),
    });

    println!(
        "{} -> {}",
        String::from("HEAD").bright_cyan(),
        current_branch.bright_green()
    );

    while !heap.is_empty() {
        let commit = match heap.pop() {
            Some(c) => c,
            None => continue,
        };

        if commit_fr.contains(&(commit.commit.timestamp, commit.hash.clone())) {
            continue;
        }
        commit_fr.insert((commit.commit.timestamp, commit.hash.clone()));

        println!("Commit: {}", commit.hash.yellow());

        if let Some(parents) = &commit.commit.parent
            && parents.len() > 1
        {
            println!("Merge: {} + {}", parents[0], parents[1]);
        }

        let data_time = match DateTime::from_timestamp(commit.commit.timestamp as i64, 0) {
            Some(t) => t,
            None => {
                println!("Invalid timestamp for commit {}", commit.hash);
                continue;
            }
        };

        let datetime: DateTime<Local> =
            DateTime::from_naive_utc_and_offset(data_time.naive_utc(), *Local::now().offset());
        let date_str = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

        println!("Date: {}", date_str);
        println!("Message: {}", commit.commit.message);
        println!();

        if let Some(parents) = commit.commit.parent {
            for parent in parents {
                commit_hash = parent;

                if !commit_hash.is_empty() {
                    heap.push(CommitDataForLog {
                        commit: match load_json(
                            &(root.root.clone() + "/.my_svn/objects/" + commit_hash.as_str()),
                        ) {
                            Ok(c) => c,
                            Err(e) => {
                                return Err(error_data!(
                                    "log",
                                    e.to_string(),
                                    "Failed to load commit object"
                                ));
                            }
                        },
                        hash: commit_hash.clone(),
                    });
                }
            }
        }
    }

    Ok(())
}
