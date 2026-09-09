use std::collections::BTreeMap;

use crate::api::schema::{
    DiffView, GitHubIssueSearchParams, GitHubIssueTaskCreateParams, TaskCreateParams,
    TaskDiffParams, TaskFileReadParams, TaskFileWriteParams, TaskGitAction, TaskGitActionParams,
    TaskListParams, TaskOpenParams, TaskRenameParams, TaskResourcesParams, TaskTarget,
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
        "resources" => task_resources(&args[1..]),
        "diff" => task_diff(&args[1..]),
        "read" => task_read(&args[1..]),
        "write" => task_write(&args[1..]),
        "github-search" => github_search(&args[1..]),
        "github-create" => github_create(&args[1..]),
        "checks" => task_checks(&args[1..]),
        "git" => task_git(&args[1..]),
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
    let mut resource_ids = Vec::new();
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
            "--resource" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                resource_ids.push(value);
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
        resource_ids,
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

fn task_resources(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task resources <task_id> [--resource RESOURCE_ID ...]");
        return Ok(2);
    };
    let mut resource_ids = Vec::new();
    let mut index = 1;
    while index < args.len() {
        if args[index] != "--resource" {
            eprintln!("unknown option: {}", args[index]);
            return Ok(2);
        }
        let Some(resource_id) = args.get(index + 1) else {
            eprintln!("missing value for --resource");
            return Ok(2);
        };
        resource_ids.push(resource_id.clone());
        index += 2;
    }
    super::runtime::task_resources(TaskResourcesParams {
        task_id: task_id.clone(),
        resource_ids,
    })
}
fn task_checks(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr task checks <task_id>");
        return Ok(2);
    }
    super::runtime::task_checks(crate::api::schema::TaskChecksParams {
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

fn task_read(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task read <task_id> --path PATH");
        return Ok(2);
    };
    let mut path = None;
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
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: herdr task read <task_id> --path PATH");
        return Ok(2);
    };
    super::runtime::task_file_read(TaskFileReadParams {
        task_id: task_id.clone(),
        path,
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

fn task_git(args: &[String]) -> std::io::Result<i32> {
    let Some(task_id) = args.first() else {
        eprintln!("usage: herdr task git <task_id> <stage|unstage|commit|push|pr> [OPTIONS]");
        return Ok(2);
    };
    let Some(action) = args.get(1).and_then(|action| match action.as_str() {
        "stage" => Some(TaskGitAction::Stage),
        "unstage" => Some(TaskGitAction::Unstage),
        "commit" => Some(TaskGitAction::Commit),
        "push" => Some(TaskGitAction::Push),
        "pr" => Some(TaskGitAction::PullRequest),
        _ => None,
    }) else {
        eprintln!("invalid Git task action");
        return Ok(2);
    };
    let mut paths = Vec::new();
    let mut message = None;
    let mut title = None;
    let mut body = None;
    let mut base = None;
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        let value = || option_value(args, index, option);
        match option {
            "--path" => {
                let Some(value) = value() else {
                    return Ok(2);
                };
                paths.push(value);
                index += 2;
            }
            "--message" => {
                message = value();
                if message.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--title" => {
                title = value();
                if title.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--body" => {
                body = value();
                if body.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--base" => {
                base = value();
                if base.is_none() {
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
    super::runtime::task_git_action(TaskGitActionParams {
        task_id: task_id.clone(),
        action,
        paths,
        message,
        title,
        body,
        base,
    })
}
fn github_search(args: &[String]) -> std::io::Result<i32> {
    let mut repository = None;
    let mut query = None;
    let mut limit = None;
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--repo" => {
                repository = option_value(args, index, option);
                if repository.is_none() {
                    return Ok(2);
                }
            }
            "--query" => {
                query = option_value(args, index, option);
                if query.is_none() {
                    return Ok(2);
                }
            }
            "--limit" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                limit = match value.parse::<u32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        eprintln!("limit must be an unsigned integer");
                        return Ok(2);
                    }
                };
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
        index += if option == "--limit" || option == "--repo" || option == "--query" {
            2
        } else {
            1
        };
    }
    let Some(repository) = repository else {
        eprintln!("usage: herdr task github-search --repo OWNER/REPO [--query TEXT] [--limit N]");
        return Ok(2);
    };
    super::runtime::github_issue_search(GitHubIssueSearchParams {
        repository,
        query,
        limit,
    })
}

fn github_create(args: &[String]) -> std::io::Result<i32> {
    let mut repository = None;
    let mut number = None;
    let mut project_id = None;
    let mut location = TaskLocationMode::Repository;
    let mut branch = None;
    let mut worktree_path = None;
    let mut prompt = None;
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--repo" => repository = option_value(args, index, option),
            "--number" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                number = match value.parse::<u64>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        eprintln!("number must be an unsigned integer");
                        return Ok(2);
                    }
                };
            }
            "--project" => project_id = option_value(args, index, option),
            "--location" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                location = match value.as_str() {
                    "repository" => TaskLocationMode::Repository,
                    "worktree" => TaskLocationMode::Worktree,
                    _ => {
                        eprintln!("location must be repository or worktree");
                        return Ok(2);
                    }
                };
            }
            "--branch" => branch = option_value(args, index, option),
            "--worktree-path" => worktree_path = option_value(args, index, option),
            "--prompt" => prompt = option_value(args, index, option),
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
        if matches!(
            option,
            "--repo"
                | "--number"
                | "--project"
                | "--location"
                | "--branch"
                | "--worktree-path"
                | "--prompt"
        ) {
            if args.get(index + 1).is_none() {
                eprintln!("missing value for {option}");
                return Ok(2);
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    let (Some(repository), Some(number), Some(project_id)) = (repository, number, project_id)
    else {
        eprintln!(
            "usage: herdr task github-create --repo OWNER/REPO --number N --project ID [OPTIONS]"
        );
        return Ok(2);
    };
    super::runtime::github_issue_create(GitHubIssueTaskCreateParams {
        repository,
        number,
        project_id,
        location,
        branch,
        worktree_path,
        prompt,
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
    println!("  resources <task_id> [--resource RESOURCE_ID ...]");
    println!("  diff <task_id> [--base REF] [--split|--unified]");
    println!("  read <task_id> --path PATH");
    println!("  write <task_id> --path PATH --content TEXT");
    println!("  git <task_id> <stage|unstage|commit|push|pr> [OPTIONS]");
    println!("  github-search --repo OWNER/REPO [--query TEXT] [--limit N]");
    println!("  github-create --repo OWNER/REPO --number N --project ID [OPTIONS]");
}
