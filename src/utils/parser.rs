use std::env;

#[derive(PartialEq)]
pub enum Command {
    Init,
    Add(Vec<String>),
    Commit(String),
    Checkout(String),
    Branch(String),
    Merge(String),
    Diff(Option<String>),
    Status,
    Log,
    Help,
}

pub fn parse_args() -> Result<Command, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Ok(Command::Help);
    }

    match args[1].as_str() {
        "init" => Ok(Command::Init),
        "log" => Ok(Command::Log),
        "status" => Ok(Command::Status),
        "add" => {
            if args.len() >= 3 {
                Ok(Command::Add(args[2..].to_vec()))
            } else {
                Err(String::from(
                    "The add command requires at least one file or directory as an argument",
                ))
            }
        }
        "commit" => {
            if args.len() >= 4 && args[2] == "-m" {
                Ok(Command::Commit(args[3].clone()))
            } else {
                Err(String::from(
                    "The commit command requires a message! (-m \"Message\")",
                ))
            }
        }
        "checkout" => {
            if args.len() >= 3 {
                Ok(Command::Checkout(args[2].clone()))
            } else {
                Err(String::from(
                    "The checkout command requires a branch name as an argument",
                ))
            }
        }
        "branch" => {
            if args.len() >= 3 {
                Ok(Command::Branch(args[2].clone()))
            } else {
                Err(String::from(
                    "The branch command requires a branch name as an argument",
                ))
            }
        }
        "merge" => {
            if args.len() >= 3 {
                Ok(Command::Merge(args[2].clone()))
            } else {
                Err(String::from(
                    "The merge command requires a branch name as an argument",
                ))
            }
        }
        "diff" => {
            if args.len() >= 3 {
                Ok(Command::Diff(Some(args[2].clone())))
            } else {
                Ok(Command::Diff(None))
            }
        }
        "help" => Ok(Command::Help),
        _ => Err(String::from("Unknown command")),
    }
}
