use crate::{error_data, utils::error::ErrorData, utils::path::get_absolute_path};
use std::{fs, path::Path};

pub fn init() -> Result<String, ErrorData> {
    if Path::new(".my_svn").exists() {
        return Ok(format!(
            "Reinitialized existing Svn repository in {}",
            get_absolute_path().map_err(|e| error_data!(
                "init",
                e.to_string(),
                "Failed to get absolute path"
            ))?
        ));
    }

    fs::create_dir(".my_svn")
        .map_err(|e| error_data!("init", e.to_string(), "Failed to create .my_svn directory"))?;

    fs::create_dir(".my_svn/objects").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to create .my_svn/objects directory"
        )
    })?;

    fs::create_dir(".my_svn/refs").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to create .my_svn/refs directory"
        )
    })?;

    fs::create_dir(".my_svn/refs/heads").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to create .my_svn/refs/heads directory"
        )
    })?;

    fs::write(".my_svn/refs/heads/main", "").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to write in .my_svn/refs/heads/main file"
        )
    })?;

    fs::write(".my_svn/HEAD", "main").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to write in .my_svn/HEAD file"
        )
    })?;

    fs::write(".my_svn/index", "{}").map_err(|e| {
        error_data!(
            "init",
            e.to_string(),
            "Failed to write in .my_svn/index file"
        )
    })?;

    Ok(format!(
        "Initialized empty Svn repository in {}",
        get_absolute_path().map_err(|e| error_data!(
            "init",
            e.to_string(),
            "Failed to get absolute path"
        ))?
    ))
}
