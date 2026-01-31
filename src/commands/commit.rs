use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    commands::branch::{get_branch_hash, get_current_branch},
    error_data,
    utils::{
        error::ErrorData,
        index::IndexData,
        json::load_json,
        path::{FileInfo, RepoLocationError::*, find_repo_root, format_path},
    },
};

#[derive(Debug)]
pub struct TreeNode {
    name: String,
    is_file: bool,
    hash: String,
    timestamp: Option<u64>,
    children: HashMap<String, Box<TreeNode>>,
}

#[derive(Serialize, Deserialize, PartialEq)]
enum TreeDataType {
    File,
    Folder,
}

#[derive(Serialize, Deserialize)]
struct TreeData {
    data_type: TreeDataType,
    name: String,
    hash: String,
    timestamp: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct CommitObject {
    tree: String,
    pub(crate) parent: Option<Vec<String>>,
    pub(crate) message: String,
    pub(crate) timestamp: u64,
}

fn travel_commit_tree(root: &String, node: &mut TreeNode) -> Result<(), ErrorData> {
    if node.is_file {
        return Ok(());
    }

    for i in node.children.values_mut() {
        travel_commit_tree(root, i)?;
    }

    let mut tree_file: Vec<TreeData> = Vec::new();
    for i in node.children.values() {
        tree_file.push(TreeData {
            data_type: (if i.is_file {
                TreeDataType::File
            } else {
                TreeDataType::Folder
            }),
            timestamp: i.timestamp,
            name: i.name.clone(),
            hash: i.hash.clone(),
        });
    }

    let json_string = match serde_json::to_string(&tree_file) {
        Ok(json) => json,
        Err(e) => {
            return Err(error_data!(
                "travel_commit_tree",
                e.to_string(),
                "Failed to serialize tree data to JSON"
            ));
        }
    };

    let mut hasher = Sha1::new();
    hasher.update(json_string.as_bytes());
    let hash = hasher.finalize();

    fs::write(
        format_path(&vec![root, ".my_svn", "objects", &hex::encode(hash)]),
        json_string,
    )
    .map_err(|e| {
        error_data!(
            "travel_commit_tree",
            e.to_string(),
            "Failed to write tree object to file"
        )
    })?;

    node.hash = hex::encode(hash);

    Ok(())
}

pub fn commit(message: String, from_merge: &str) -> Result<(), ErrorData> {
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

    let index_files = match IndexData::new() {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "Failed to load index data"
            ));
        }
    };

    let mut head: TreeNode = TreeNode {
        name: String::new(),
        is_file: false,
        hash: String::new(),
        timestamp: None,
        children: HashMap::new(),
    };

    for (path, info) in index_files.entries {
        let mut temp_head = &mut head;

        for path_component in Path::new(&path)
            .components()
            .filter_map(|c| c.as_os_str().to_str())
        {
            temp_head = temp_head
                .children
                .entry(path_component.to_string())
                .or_insert_with(|| {
                    Box::new(TreeNode {
                        name: path_component.to_string(),
                        is_file: false,
                        hash: String::new(),
                        timestamp: None,
                        children: HashMap::new(),
                    })
                })
                .as_mut();
        }

        temp_head.timestamp = Some(info.timestamp);
        temp_head.is_file = true;
        temp_head.hash = info.hash;
    }

    match travel_commit_tree(&root.root, &mut head) {
        Ok(_) => {}
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "Failed to travel commit tree"
            ));
        }
    }

    let commit_parent;
    let current_branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };

    match get_branch_hash(&root.root, &current_branch) {
        Ok(h) => {
            if from_merge.is_empty() {
                commit_parent = Some(vec![h]);
            } else {
                commit_parent = Some(vec![h, from_merge.to_string()]);
            }
        }
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "Failed to get branch hash"
            ));
        }
    }

    let start = SystemTime::now();

    let timestamp_duration = match start.duration_since(UNIX_EPOCH) {
        Ok(n) => n,
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "SystemTime before UNIX EPOCH!"
            ));
        }
    };

    let commit_obj: CommitObject = CommitObject {
        tree: head.hash,
        parent: commit_parent,
        message,
        timestamp: timestamp_duration.as_secs(),
    };
    let json_string = match serde_json::to_string(&commit_obj) {
        Ok(j) => j,
        Err(e) => {
            return Err(error_data!(
                "commit",
                e.to_string(),
                "Failed to serialize commit object to JSON"
            ));
        }
    };

    let mut hasher = Sha1::new();
    hasher.update(json_string.as_bytes());
    let hash = hasher.finalize();

    fs::write(
        format_path(&vec![&root.root, ".my_svn", "objects", &hex::encode(hash)]),
        json_string,
    )
    .map_err(|e| {
        error_data!(
            "commit",
            e.to_string(),
            "Failed to write commit object to file"
        )
    })?;

    fs::write(
        format_path(&vec![
            &root.root,
            ".my_svn",
            "refs",
            "heads",
            &current_branch,
        ]),
        hex::encode(hash),
    )
    .map_err(|e| {
        error_data!(
            "commit",
            e.to_string(),
            "Failed to write commit reference to file"
        )
    })?;

    Ok(())
}

fn read_commit_data_rec(
    objects_path: &String,
    hash: String,
    path: String,
    data: &mut HashMap<String, FileInfo>,
) -> Result<(), ErrorData> {
    let content: Vec<TreeData> = match load_json(&format_path(&vec![objects_path, &hash])) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "read_commit_data_rec",
                e.to_string(),
                "Failed to load tree object"
            ));
        }
    };

    for i in content {
        let new_path = if path.is_empty() {
            i.name
        } else {
            format_path(&vec![&path, &i.name])
        };

        if i.data_type == TreeDataType::Folder {
            read_commit_data_rec(objects_path, i.hash, new_path, data)?;
        } else {
            data.insert(
                new_path,
                FileInfo {
                    hash: i.hash,
                    timestamp: i.timestamp.unwrap_or(0),
                },
            );
        }
    }

    Ok(())
}

pub fn read_commit(root: &str, branch: &str) -> Result<HashMap<String, FileInfo>, ErrorData> {
    let mut commit_data: HashMap<String, FileInfo> = HashMap::new();

    let tree_hash =
        match fs::read_to_string(format_path(&vec![root, ".my_svn", "refs", "heads", branch])) {
            Ok(h) => h,
            Err(e) => {
                return Err(error_data!(
                    "read_commit",
                    e.to_string(),
                    "Failed to read branch hash"
                ));
            }
        };

    if tree_hash.is_empty() {
        return Ok(commit_data);
    }

    let tree_root: CommitObject =
        match load_json(&format_path(&vec![root, ".my_svn", "objects", &tree_hash])) {
            Ok(data) => data,
            Err(e) => {
                return Err(error_data!(
                    "read_commit",
                    e.to_string(),
                    "Failed to load commit object"
                ));
            }
        };

    match read_commit_data_rec(
        &format_path(&vec![root, ".my_svn", "objects"]),
        tree_root.tree,
        String::new(),
        &mut commit_data,
    ) {
        Ok(_) => (),
        Err(e) => {
            return Err(error_data!(
                "read_commit",
                e.to_string(),
                "Failed to read commit data recursively"
            ));
        }
    }

    Ok(commit_data)
}

pub fn find_base_commit(
    current_commit: &String,
    target_commit: &str,
    root: &String,
) -> Result<Option<String>, ErrorData> {
    let mut current_parents: HashSet<String> = HashSet::new();
    let mut temp_commit = current_commit.to_owned();
    let mut parent_queue: VecDeque<String> = VecDeque::new();

    while !temp_commit.is_empty() {
        current_parents.insert(temp_commit.clone());

        let commit_obj: CommitObject = match load_json(&format_path(&vec![
            &root,
            ".my_svn",
            "objects",
            &temp_commit,
        ])) {
            Ok(data) => data,
            Err(e) => {
                return Err(error_data!(
                    "find_base_commit",
                    e.to_string(),
                    "Failed to load commit object"
                ));
            }
        };

        if let Some(parent) = commit_obj.parent {
            temp_commit = parent[0].clone();

            if parent.len() > 1 {
                for i in parent.iter().skip(1) {
                    parent_queue.push_back(i.clone());
                }
            }
        } else if !parent_queue.is_empty() {
            temp_commit = String::new();
            while !parent_queue.is_empty() && temp_commit.is_empty() {
                temp_commit = parent_queue.pop_front().unwrap_or_default();
            }
        } else {
            break;
        }
    }

    temp_commit = target_commit.to_owned();
    parent_queue.clear();
    parent_queue.push_back(temp_commit);

    while !parent_queue.is_empty() {
        temp_commit = match parent_queue.pop_front() {
            Some(c) => c,
            None => continue,
        };

        if current_parents.contains(&temp_commit) {
            return Ok(Some(temp_commit));
        }

        let commit_obj: CommitObject = match load_json(&format_path(&vec![
            &root,
            ".my_svn",
            "objects",
            &temp_commit,
        ])) {
            Ok(data) => data,
            Err(e) => {
                return Err(error_data!(
                    "find_base_commit",
                    e.to_string(),
                    "Failed to load commit object"
                ));
            }
        };

        if let Some(parent) = commit_obj.parent {
            for i in parent {
                parent_queue.push_back(i.clone());
            }
        }
    }

    Ok(None)
}

pub fn read_commit_from_hash(
    root: &String,
    commit_hash: &str,
) -> Result<HashMap<String, FileInfo>, ErrorData> {
    let mut commit_data: HashMap<String, FileInfo> = HashMap::new();

    if commit_hash.is_empty() {
        return Ok(commit_data);
    }

    let tree_root: CommitObject = match load_json(&format_path(&vec![
        &root,
        ".my_svn",
        "objects",
        &commit_hash,
    ])) {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "read_commit_from_hash",
                e.to_string(),
                "Failed to load commit object"
            ));
        }
    };

    match read_commit_data_rec(
        &(root.to_owned() + "/.my_svn/objects/"),
        tree_root.tree,
        String::new(),
        &mut commit_data,
    ) {
        Ok(_) => (),
        Err(e) => {
            return Err(error_data!(
                "read_commit_from_hash",
                e.to_string(),
                "Failed to read commit data recursively"
            ));
        }
    }

    Ok(commit_data)
}
