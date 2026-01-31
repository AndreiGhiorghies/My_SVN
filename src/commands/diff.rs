use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use colored::Colorize;

use crate::commands::branch::{get_branch_hash, get_current_branch};
use crate::commands::commit::CommitObject;
use crate::utils::json::load_json;
use crate::utils::path::RepoLocationError::*;
use crate::{
    commands::commit::read_commit_from_hash,
    error_data,
    utils::{
        error::ErrorData,
        path::{find_repo_root, format_path},
    },
};

struct FileView {
    data: Vec<u8>,
    line_hashes: Vec<u64>,
    line_offsets: Vec<(usize, usize)>, // (Start, End) pt fiecare linie
}

impl FileView {
    fn new(filepath: &str) -> std::io::Result<Self> {
        let data = std::fs::read(filepath)?;

        let mut hashes = Vec::new();
        let mut offsets = Vec::new();

        let mut start = 0;
        for (i, &byte) in data.iter().enumerate() {
            if byte == b'\n' {
                let line_slice = &data[start..i]; // Slice safe

                let mut hasher = DefaultHasher::new();
                line_slice.hash(&mut hasher);
                hashes.push(hasher.finish());

                offsets.push((start, i));
                start = i + 1;
            }
        }

        if start < data.len() {
            let line_slice = &data[start..];
            let mut hasher = DefaultHasher::new();
            line_slice.hash(&mut hasher);
            hashes.push(hasher.finish());
            offsets.push((start, data.len()));
        }

        Ok(FileView {
            data,
            line_hashes: hashes,
            line_offsets: offsets,
        })
    }

    fn get_line(&self, index: usize) -> &str {
        let (start, end) = self.line_offsets[index];

        let bytes = &self.data[start..end];
        std::str::from_utf8(bytes).unwrap_or_default()
    }

    fn len(&self) -> usize {
        self.line_hashes.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DiffChange {
    Insert(String),
    Delete(String),
}

fn myers_diff(old_lines: &FileView, new_lines: &FileView) -> Vec<DiffChange> {
    let n = old_lines.line_hashes.len() as isize;
    let m = new_lines.line_hashes.len() as isize;
    let max = n + m;

    // 'v' stochează valoarea maximă a lui x pentru fiecare diagonală k.
    // Diagonala k = x - y.
    // Deoarece k poate fi negativ (-m la n), folosim un array cu offset sau un HashMap.
    let mut v: HashMap<isize, isize> = HashMap::new();

    // Inițializare: pe diagonala 1, x începe de la 0.
    v.insert(1, 0);

    // Stocăm (snapshots ale vectorului V pentru a putea face backtracking.
    let mut trace: Vec<HashMap<isize, isize>> = Vec::new();

    // 'd' este numărul de modificări (costul).
    for d in 0..=max {
        trace.push(v.clone());

        // Căutăm pe diagonalele active la pasul d.
        // k merge din 2 în 2 (paritatea se păstrează).
        let start_k = -d;
        let end_k = d;

        let mut k = start_k;
        while k <= end_k {
            // Alegem mutarea optimă:
            // Putem ajunge pe diagonala k fie de la k-1 (coborând/insert), fie de la k+1 (dreapta/delete).
            // Alegem varianta care ne duce la un x mai mare (progresăm mai mult în textul vechi).

            let x_down = match v.get(&(k - 1)) {
                Some(x) => *x,
                None => -1,
            };
            let x_right = match v.get(&(k + 1)) {
                Some(x) => *x,
                None => -1,
            };

            let x_start = if k == -d || (k != d && x_down < x_right) {
                // Venim de la k+1 (Delete) -> Mutare Dreapta
                x_right
            } else {
                // Venim de la k-1 (Insert) -> Mutare Jos
                x_down + 1
            };

            let mut x = x_start;
            let mut y = x - k;

            // Cat timp liniile sunt identice, avansam pe diagonală (cost 0)
            while x < n
                && y < m
                && old_lines.line_hashes[x as usize] == new_lines.line_hashes[y as usize]
            {
                x += 1;
                y += 1;
            }

            v.insert(k, x);

            // Dacă am ajuns la final (dreapta-jos)
            if x >= n && y >= m {
                // Am găsit drumul! Acum facem backtracking.
                return backtrack(trace, old_lines, new_lines);
            }

            k += 2;
        }
    }

    vec![]
}

//Reconstrucția diff-ului
fn backtrack(
    trace: Vec<HashMap<isize, isize>>,
    old_lines: &FileView,
    new_lines: &FileView,
) -> Vec<DiffChange> {
    let mut result = Vec::new();
    let mut x = old_lines.len() as isize;
    let mut y = new_lines.len() as isize;

    for d in (0..trace.len()).rev() {
        let v = &trace[d];
        let k = x - y;

        let prev_k = if k == -(d as isize)
            || (k != (d as isize)
                && match v.get(&(k - 1)) {
                    Some(val) => *val,
                    None => -1,
                } < match v.get(&(k + 1)) {
                    Some(val) => *val,
                    None => -1,
                }) {
            k + 1 // Am venit din Delete
        } else {
            k - 1 // Am venit din Insert
        };

        let prev_x = match v.get(&prev_k) {
            Some(val) => *val,
            None => -1,
        };
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
        }

        if d > 0 {
            if x == prev_x {
                // x nu s-a schimbat, dar y a scăzut -> Înseamnă că am urcat (opusul lui Insert/Jos)
                result.push(DiffChange::Insert(
                    new_lines.get_line((y - 1) as usize).to_string(),
                ));
                y -= 1;
            } else if y == prev_y {
                // y nu s-a schimbat, dar x a scăzut -> Înseamnă că am mers stânga (opusul lui Delete/Dreapta)
                result.push(DiffChange::Delete(
                    old_lines.get_line((x - 1) as usize).to_string(),
                ));
                x -= 1;
            }
        }
    }

    // Rezultatul este inversat (din cauza backtracking-ului), îl întoarcem
    result.reverse();
    result
}

//first_hash = commit-ul curent / second_hash = commit-ul cu care se face diff
fn diff_between_hash(first_hash: &str, second_hash: &str, root: &str) -> Result<(), ErrorData> {
    let first_commit = match read_commit_from_hash(&root.to_string(), first_hash) {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "diff",
                e.to_string(),
                "Error reading commit for diff"
            ));
        }
    };
    let mut second_commit = match read_commit_from_hash(&root.to_string(), second_hash) {
        Ok(data) => data,
        Err(e) => {
            return Err(error_data!(
                "diff",
                e.to_string(),
                "Error reading commit for diff"
            ));
        }
    };

    let mut diff_found = false;

    for (path, info) in first_commit.iter() {
        if second_commit.contains_key(path) {
            if second_commit[path].hash != info.hash {
                println!("File {} was modified:", path.yellow());

                let mut tabs: String = String::new();
                for _ in 0..(path.len() + String::from("File  was modified:").len()) {
                    tabs.push(' ');
                }

                let second_commit_file = match FileView::new(&format_path(&vec![
                    &root,
                    ".my_svn",
                    "objects",
                    &second_commit[path].hash,
                ])) {
                    Ok(data) => data,
                    Err(e) => {
                        return Err(error_data!(
                            "diff",
                            e.to_string(),
                            "Error opening file for diff"
                        ));
                    }
                };

                let first_commit_file = match FileView::new(&format_path(&vec![
                    &root, ".my_svn", "objects", &info.hash,
                ])) {
                    Ok(data) => data,
                    Err(e) => {
                        return Err(error_data!(
                            "diff",
                            e.to_string(),
                            "Error opening file for diff"
                        ));
                    }
                };

                let diffs = myers_diff(&second_commit_file, &first_commit_file);
                diff_found = diff_found || !diffs.is_empty();
                for diff in diffs {
                    match diff {
                        DiffChange::Insert(line) => {
                            println!("{}{}", tabs, format!("+{}", line).green());
                        }
                        DiffChange::Delete(line) => {
                            println!("{}{}", tabs, format!("-{}", line).red());
                        }
                    }
                }
            }

            second_commit.remove(path);
        } else {
            diff_found = true;
            println!("File {} was added.", path.green());
        }
    }

    for (path, _) in second_commit.iter() {
        println!("File {} was deleted.", path.red());
    }

    if !diff_found && second_commit.is_empty() {
        println!("No differences found between the specified commits.");
    }

    Ok(())
}

pub fn diff(commit: Option<String>) -> Result<(), ErrorData> {
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

    let branch = match get_current_branch(&root.root) {
        Ok(b) => b,
        Err(e) => {
            return Err(error_data!(
                "diff",
                e.to_string(),
                "Failed to get current branch"
            ));
        }
    };
    let current_commit_hash = match get_branch_hash(&root.root, &branch) {
        Ok(h) => h,
        Err(e) => {
            return Err(error_data!(
                "diff",
                e.to_string(),
                "Failed to get current commit hash"
            ));
        }
    };

    if current_commit_hash.is_empty() {
        println!("No commits found on the current branch to diff.");
        return Ok(());
    }

    if let Some(commit_name) = commit {
        let target_commit_hash = if Path::new(&format_path(&vec![
            &root.root,
            ".my_svn",
            "refs",
            "heads",
            &commit_name,
        ]))
        .exists()
        {
            get_branch_hash(&root.root, &commit_name).map_err(|e| {
                error_data!("diff", e.to_string(), "Failed to get target commit hash")
            })?
        } else {
            println!("fatal: A branch named '{}' does not exist.", commit_name);
            return Ok(());
        };

        if current_commit_hash == target_commit_hash {
            println!("No differences between the same commit.");
            return Ok(());
        }

        diff_between_hash(&current_commit_hash, &target_commit_hash, &root.root)
            .map_err(|e| error_data!("diff", e.to_string(), "Error during diff between commits"))?;
    } else {
        let current_commit = match load_json::<CommitObject>(&format_path(&vec![
            &root.root,
            ".my_svn",
            "objects",
            &current_commit_hash,
        ])) {
            Ok(c) => c,
            Err(e) => {
                return Err(error_data!(
                    "diff",
                    e.to_string(),
                    "Failed to read current commit"
                ));
            }
        };

        let parent_commit_hash = match &current_commit.parent {
            Some(parents) => parents[0].clone(),
            None => {
                println!("No parent commit to diff against.");
                return Ok(());
            }
        };

        diff_between_hash(&current_commit_hash, &parent_commit_hash, &root.root)
            .map_err(|e| error_data!("diff", e.to_string(), "Error during diff between commits"))?;
    }

    Ok(())
}
