use crate::api::schema::{
    ProjectCreateParams, ProjectOpenParams, ProjectRenameParams, ProjectTarget,
};
use std::collections::BTreeMap;

pub(super) fn run_project_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(|arg| arg.as_str()) else {
        print_project_help();
        return Ok(2);
    };

    match subcommand {
        "list" => project_list(&args[1..]),
        "create" => project_create(&args[1..]),
        "get" => project_get(&args[1..]),
        "open" => project_open(&args[1..]),
        "rename" => project_rename(&args[1..]),
        "delete" => project_delete(&args[1..]),
        "help" | "--help" | "-h" => {
            print_project_help();
            Ok(0)
        }
        _ => {
            print_project_help();
            Ok(2)
        }
    }
}

fn project_list(args: &[String]) -> std::io::Result<i32> {
    if !args.is_empty() {
        eprintln!("usage: herdr project list");
        return Ok(2);
    }
    super::runtime::project_list()
}

fn project_create(args: &[String]) -> std::io::Result<i32> {
    let mut name = None;
    let mut root_path = None;
    let mut open = false;
    let mut focus = true;
    let mut environment = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --name");
                    return Ok(2);
                };
                name = Some(value.clone());
                index += 2;
            }
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --path");
                    return Ok(2);
                };
                root_path = Some(value.clone());
                index += 2;
            }
            "--env" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --env");
                    return Ok(2);
                };
                let Some((name, value)) = value.split_once('=') else {
                    eprintln!("environment must use NAME=VALUE");
                    return Ok(2);
                };
                environment.insert(name.to_owned(), value.to_owned());
                index += 2;
            }
            "--open" => {
                open = true;
                index += 1;
            }
            "--focus" => {
                focus = true;
                index += 1;
            }
            "--no-focus" => {
                focus = false;
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let (Some(name), Some(root_path)) = (name, root_path) else {
        eprintln!(
            "usage: herdr project create --name NAME --path PATH [--open] [--focus|--no-focus]"
        );
        return Ok(2);
    };
    super::runtime::project_create(ProjectCreateParams {
        name,
        root_path,
        open,
        focus: open && focus,
        worktree_root: None,
        worktree_base: None,
        default_agent: None,
        preserve_patterns: None,
        lifecycle: None,
        environment: (!environment.is_empty()).then_some(environment),
    })
}

fn project_get(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr project get <project_id>");
        return Ok(2);
    }
    super::runtime::project_get(ProjectTarget {
        project_id: args[0].clone(),
    })
}

fn project_open(args: &[String]) -> std::io::Result<i32> {
    let Some(project_id) = args.first() else {
        eprintln!("usage: herdr project open <project_id> [--focus|--no-focus]");
        return Ok(2);
    };
    let mut focus = true;
    for option in &args[1..] {
        match option.as_str() {
            "--focus" => focus = true,
            "--no-focus" => focus = false,
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    super::runtime::project_open(ProjectOpenParams {
        project_id: project_id.clone(),
        focus,
    })
}

fn project_rename(args: &[String]) -> std::io::Result<i32> {
    if args.len() < 2 {
        eprintln!("usage: herdr project rename <project_id> <name>");
        return Ok(2);
    }
    super::runtime::project_rename(ProjectRenameParams {
        project_id: args[0].clone(),
        name: args[1..].join(" "),
    })
}

fn project_delete(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr project delete <project_id>");
        return Ok(2);
    }
    super::runtime::project_delete(ProjectTarget {
        project_id: args[0].clone(),
    })
}

fn print_project_help() {
    println!("herdr project — manage persistent project registrations");
    println!();
    println!("usage: herdr project <subcommand>");
    println!("  list");
    println!("  create --name NAME --path PATH [--open] [--focus|--no-focus]");
    println!("  get <project_id>");
    println!("  open <project_id> [--focus|--no-focus]");
    println!("  rename <project_id> <name>");
    println!("  delete <project_id>");
}
