use std::collections::BTreeMap;

use crate::api::schema::{
    DiffView, TaskCreateParams, TaskDiffParams, TaskFileWriteParams, TaskListParams,
    TaskOpenParams, TaskRenameParams, TaskTarget,
};
use crate::task::TaskLocationMode;

pub(super) fn run_task_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_task_help();
        return Ok(2);
    };

    match subcommand {
        "list" => task_list(&args[1..]),
        "create" => task_create(&args[1..]),
        "open" => task_open(&args[1..]),
        "rename" => task_rename(&args[1..]),
        "close" => task_close(&args[1..]),
        "diff" => task_diff(&args[1..]),
        "write" => task_write(&args[1..]),
        "help" | "--help" | "-h" => {
            print_task_help();
            Ok(0)
        }
        _ => {
            print_task_help();
            Ok(2)
        }
    }
}

fn task_list(args: &[String]) -> std::io::Result<i32> {
    let mut project_id = None;
    let mut include_closed = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--project" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --project");
                    return Ok(2);
                };
                project_id = Some(value.clone());
                index += 2;
            }
            "--include-closed" => {
                include_closed = true;
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    super::runtime::task_list(TaskListParams {
        project_id,
        include_closed,
    })
}

fn option_value(args: &[String], index: usize, option: &str) -> Option<String> {
    args.get(index + 1).cloned().or_else(|| {
        eprintln!("missing value for {option}");
        None
    })
}

fn task_create(args: &[String]) -> std::io::Result<i32> {
    let mut project_id = None;
    let mut name = None;
    let mut location = TaskLocationMode::Repository;
    let mut branch = None;
    let mut worktree_path = None;
    let mut provider = None;
    let mut model = None;
    let mut prompt = None;
    let mut workspace_id = None;
    let mut tab_id = None;
    let mut pane_id = None;
    let mut environment = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--project" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                project_id = Some(value);
                index += 2;
            }
            "--name" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                name = Some(value);
                index += 2;
            }
            "--location" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                location = match value.as_str() {
                    "repository" => TaskLocationMode::Repository,
                    "worktree" => TaskLocationMode::Worktree,
                    other => {
                        eprintln!("invalid task location: {other}");
                        return Ok(2);
                    }
                };
                index += 2;
            }
            "--branch" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                branch = Some(value);
                index += 2;
            }
            "--worktree-path" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                worktree_path = Some(value);
                index += 2;
            }
            "--provider" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                provider = Some(value);
                index += 2;
            }
            "--env" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                let Some((name, value)) = value.split_once('=') else {
                    eprintln!("environment must use NAME=VALUE");
                    return Ok(2);
                };
                environment.insert(name.to_owned(), value.to_owned());
                index += 2;
            }
            "--model" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                model = Some(value);
                index += 2;
            }
            "--prompt" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                prompt = Some(value);
                index += 2;
            }
            "--workspace-id" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                workspace_id = Some(value);
                index += 2;
            }
            "--tab-id" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                tab_id = Some(value);
                index += 2;
            }
            "--pane-id" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                pane_id = Some(value);
                index += 2;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let (Some(project_id), Some(name)) = (project_id, name) else {
        eprintln!("usage: herdr task create --project ID --name NAME [OPTIONS]");
        return Ok(2);
    };
    super::runtime::task_create(TaskCreateParams {
        project_id,
        name,
        location,
        branch,
        worktree_path,
        provider,
        model,
        prompt,
        environment: (!environment.is_empty()).then_some(environment),
        workspace_id,
        tab_id,
        pane_id,
    })
}

fn task_open(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task open <task_id> [--focus|--no-focus]");
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
    super::runtime::task_open(TaskOpenParams {
        task_id: task_id.clone(),
        focus,
    })
}

fn task_rename(args: &[String]) -> std::io::Result<i32> {
    if args.len() < 2 {
        eprintln!("usage: herdr task rename <task_id> <name>");
        return Ok(2);
    }
    super::runtime::task_rename(TaskRenameParams {
        task_id: args[0].clone(),
        name: args[1..].join(" "),
    })
}

fn task_close(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr task close <task_id>");
        return Ok(2);
    }
    super::runtime::task_close(TaskTarget {
        task_id: args[0].clone(),
    })
}
fn task_diff(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task diff <task_id> [--base REF] [--split|--unified]");
        return Ok(2);
    };
    let mut base = None;
    let mut view = DiffView::Unified;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--base" => {
                let Some(value) = option_value(args, index, "--base") else {
                    return Ok(2);
                };
                base = Some(value);
                index += 2;
            }
            "--split" => {
                view = DiffView::Split;
                index += 1;
            }
            "--unified" => {
                view = DiffView::Unified;
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    super::runtime::task_diff(TaskDiffParams {
        task_id: task_id.clone(),
        base,
        view,
    })
}

fn task_write(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task write <task_id> --path PATH --content TEXT");
        return Ok(2);
    };
    let mut path = None;
    let mut content = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => {
                path = option_value(args, index, "--path");
                if path.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--content" => {
                content = option_value(args, index, "--content");
                if content.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let (Some(path), Some(content)) = (path, content) else {
        eprintln!("usage: herdr task write <task_id> --path PATH --content TEXT");
        return Ok(2);
    };
    super::runtime::task_file_write(TaskFileWriteParams {
        task_id: task_id.clone(),
        path,
        content,
    })
}

fn print_task_help() {
    println!("herdr task — manage durable agent tasks");
    println!();
    println!("usage: herdr task <subcommand>");
    println!("  list [--project ID] [--include-closed]");
    println!("  create --project ID --name NAME [OPTIONS]");
    println!("  open <task_id> [--focus|--no-focus]");
    println!("  rename <task_id> <name>");
    println!("  close <task_id>");
    println!("  diff <task_id> [--base REF] [--split|--unified]");
    println!("  write <task_id> --path PATH --content TEXT");
}
