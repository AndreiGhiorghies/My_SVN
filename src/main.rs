use colored::*;

use crate::utils::parser::{Command, parse_args};

mod commands;
mod utils;

fn main() {
    match parse_args() {
        Ok(cmd) => match cmd {
            Command::Init => match crate::commands::init::init() {
                Ok(msg) => println!("{}", msg),
                Err(e) => println!("{}{}", String::from("Error at init:\n").red(), e),
            },
            Command::Add(files) => match crate::commands::add::add(&files) {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at add:\n").red(), e),
            },
            Command::Status => match crate::commands::status::status() {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at status:\n").red(), e),
            },
            Command::Commit(message) => match crate::commands::commit::commit(message, "") {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at commit:\n").red(), e),
            },
            Command::Branch(branch_name) => {
                match crate::commands::branch::create_branch(&branch_name) {
                    Ok(_) => {}
                    Err(e) => println!("{}{}", String::from("Error at branch:\n").red(), e),
                }
            }
            Command::Checkout(branch) => match crate::commands::checkout::checkout(&branch) {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at checkout:\n").red(), e),
            },
            Command::Merge(branch) => match crate::commands::merge::merge(&branch) {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at merge:\n").red(), e),
            },
            Command::Log => match crate::commands::log::log() {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at log:\n").red(), e),
            },
            Command::Diff(commit) => match crate::commands::diff::diff(commit) {
                Ok(_) => {}
                Err(e) => println!("{}{}", String::from("Error at diff:\n").red(), e),
            },
            Command::Help => {
                println!("My_SVN - A simple version control system");
                println!();
                println!("Available commands:");
                println!("  init                 Initialize a new repository");
                println!("  add <files>         Add files to the staging area");
                println!("  commit -m <message> Commit staged changes with a message");
                println!("  status              Show the status of the working directory");
                println!("  branch <name>      Create a new branch");
                println!("  checkout <branch>  Switch to a different branch");
                println!("  merge <branch>     Merge a branch into the current branch");
                println!("  log                 Show commit history");
                println!(
                    "  diff [commit]      Show differences between commits or working directory"
                );
                println!("  help                Show this help message");
            }
        },
        Err(e) => println!("Eroare: {}", e),
    }
}
