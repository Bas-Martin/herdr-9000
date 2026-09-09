use crate::api::schema::{
    AutomationCreateParams, AutomationListParams, AutomationRunNowParams, AutomationTarget,
    AutomationUpdateParams,
};
use crate::task::TaskLocationMode;

pub(super) fn run_automation_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_automation_help();
        return Ok(2);
    };
    match subcommand {
        "list" => automation_list(&args[1..]),
        "create" => automation_create(&args[1..]),
        "update" => automation_update(&args[1..]),
        "delete" => automation_delete(&args[1..]),
        "run-now" => automation_run_now(&args[1..]),
        "help" | "--help" | "-h" => {
            print_automation_help();
            Ok(0)
        }
        _ => {
            print_automation_help();
            Ok(2)
        }
    }
}

fn option_value(args: &[String], index: usize, option: &str) -> Option<String> {
    args.get(index + 1).cloned().or_else(|| {
        eprintln!("missing value for {option}");
        None
    })
}

fn parse_location(value: &str) -> Option<TaskLocationMode> {
    match value {
        "repository" => Some(TaskLocationMode::Repository),
        "worktree" => Some(TaskLocationMode::Worktree),
        _ => None,
    }
}

fn automation_list(args: &[String]) -> std::io::Result<i32> {
    let mut project_id = None;
    let mut include_disabled = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--project" => {
                project_id = option_value(args, index, "--project");
                if project_id.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--include-disabled" => {
                include_disabled = true;
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    super::runtime::automation_list(AutomationListParams {
        project_id,
        include_disabled,
    })
}

fn automation_create(args: &[String]) -> std::io::Result<i32> {
    let mut name = None;
    let mut project_id = None;
    let mut cron = None;
    let mut prompt = None;
    let mut provider = None;
    let mut model = None;
    let mut workspace_mode = TaskLocationMode::Repository;
    let mut enabled = true;
    let mut paused = false;
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--name" => {
                name = option_value(args, index, option);
                if name.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--project" => {
                project_id = option_value(args, index, option);
                if project_id.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--cron" => {
                cron = option_value(args, index, option);
                if cron.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--prompt" => {
                prompt = option_value(args, index, option);
                if prompt.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--provider" => {
                provider = option_value(args, index, option);
                if provider.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--model" => {
                model = option_value(args, index, option);
                if model.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--workspace-mode" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                let Some(value) = parse_location(&value) else {
                    eprintln!("invalid workspace mode: {value}");
                    return Ok(2);
                };
                workspace_mode = value;
                index += 2;
            }
            "--disable" => {
                enabled = false;
                index += 1;
            }
            "--pause" => {
                paused = true;
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let (Some(name), Some(project_id), Some(cron), Some(prompt)) = (name, project_id, cron, prompt)
    else {
        eprintln!(
            "usage: herdr automation create --name NAME --project ID --cron EXPR --prompt TEXT [OPTIONS]"
        );
        return Ok(2);
    };
    super::runtime::automation_create(AutomationCreateParams {
        name,
        project_id,
        cron,
        prompt,
        provider,
        model,
        workspace_mode,
        enabled,
        paused,
    })
}

fn automation_update(args: &[String]) -> std::io::Result<i32> {
    let Some(automation_id) = args.first() else {
        eprintln!("usage: herdr automation update <automation_id> [OPTIONS]");
        return Ok(2);
    };
    let mut name = None;
    let mut cron = None;
    let mut prompt = None;
    let mut provider = None;
    let mut model = None;
    let mut workspace_mode = None;
    let mut enabled = None;
    let mut paused = None;
    let mut index = 1;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--name" => name = option_value(args, index, option),
            "--cron" => cron = option_value(args, index, option),
            "--prompt" => prompt = option_value(args, index, option),
            "--provider" => provider = option_value(args, index, option),
            "--model" => model = option_value(args, index, option),
            "--workspace-mode" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                workspace_mode = parse_location(&value);
                if workspace_mode.is_none() {
                    eprintln!("invalid workspace mode: {value}");
                    return Ok(2);
                }
            }
            "--enable" => enabled = Some(true),
            "--disable" => enabled = Some(false),
            "--pause" => paused = Some(true),
            "--resume" => paused = Some(false),
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
        if matches!(
            option,
            "--name" | "--cron" | "--prompt" | "--provider" | "--model" | "--workspace-mode"
        ) {
            if name.is_none() && option == "--name"
                || cron.is_none() && option == "--cron"
                || prompt.is_none() && option == "--prompt"
                || provider.is_none() && option == "--provider"
                || model.is_none() && option == "--model"
            {
                return Ok(2);
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    super::runtime::automation_update(AutomationUpdateParams {
        automation_id: automation_id.clone(),
        name,
        cron,
        prompt,
        provider,
        model,
        workspace_mode,
        enabled,
        paused,
    })
}

fn automation_delete(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr automation delete <automation_id>");
        return Ok(2);
    }
    super::runtime::automation_delete(AutomationTarget {
        automation_id: args[0].clone(),
    })
}

fn automation_run_now(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr automation run-now <automation_id>");
        return Ok(2);
    }
    super::runtime::automation_run_now(AutomationRunNowParams {
        automation_id: args[0].clone(),
    })
}

fn print_automation_help() {
    println!("herdr automation — manage recurring agent automations");
    println!();
    println!("usage: herdr automation <subcommand>");
    println!("  list [--project ID] [--include-disabled]");
    println!("  create --name NAME --project ID --cron EXPR --prompt TEXT [OPTIONS]");
    println!("  update <automation_id> [OPTIONS]");
    println!("  delete <automation_id>");
    println!("  run-now <automation_id>");
}
