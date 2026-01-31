use std::{fs, path::Path};

use crate::{
    error_data,
    utils::{
        error::ErrorData,
        path::{RepoLocationError::*, find_repo_root, format_path},
    },
};

pub fn get_current_branch(root: &str) -> Result<String, ErrorData> {
    let head_path = format_path(&vec![root, ".my_svn", "HEAD"]);
    let content = match fs::read_to_string(&head_path) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "get_branch",
                e.to_string(),
                "Failed to read HEAD file"
            ));
        }
    };

    let branch_name = content.trim().to_string();
    Ok(branch_name)
}

pub fn get_branch_hash(root: &str, branch: &str) -> Result<String, ErrorData> {
    let branch_path = format_path(&vec![root, ".my_svn", "refs", "heads", branch]);
    let content = match fs::read_to_string(&branch_path) {
        Ok(c) => c,
        Err(e) => {
            return Err(error_data!(
                "get_branch_hash",
                e.to_string(),
                "Failed to read branch file"
            ));
        }
    };

    let branch_hash = content.trim().to_string();
    Ok(branch_hash)
}

pub fn create_branch(branch_name: &str) -> Result<(), ErrorData> {
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
                "create_branch",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };
    let commit_hash = match get_branch_hash(&root.root, &current_branch) {
        Ok(h) => h,
        Err(e) => {
            return Err(error_data!(
                "create_branch",
                e.to_string(),
                "Failed to get branch hash"
            ));
        }
    };

    if Path::new(&format_path(&vec![
        &root.root,
        ".my_svn",
        "refs",
        "heads",
        &branch_name,
    ]))
    .exists()
    {
        println!("fatal: A branch named '{}' already exists.", branch_name);
        return Ok(());
    }

    fs::write(
        format_path(&vec![&root.root, ".my_svn", "refs", "heads", &branch_name]),
        commit_hash,
    )
    .map_err(|e| {
        error_data!(
            "create_branch",
            e.to_string(),
            "Failed to create branch file"
        )
    })?;

    Ok(())
}
