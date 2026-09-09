use crate::api::schema::{
    ResourceCreateParams, ResourceListParams, ResourceTarget, ResourceUpdateParams,
};
use crate::resource::{ResourceKind, ResourceScope};

pub(super) fn run_resource_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_resource_help();
        return Ok(2);
    };
    match subcommand {
        "list" => resource_list(&args[1..]),
        "create" => resource_create(&args[1..]),
        "update" => resource_update(&args[1..]),
        "delete" => resource_delete(&args[1..]),
        "help" | "--help" | "-h" => {
            print_resource_help();
            Ok(0)
        }
        _ => {
            print_resource_help();
            Ok(2)
        }
    }
}

fn resource_list(args: &[String]) -> std::io::Result<i32> {
    let mut project_id = None;
    let mut task_id = None;
    let mut include_disabled = false;
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
            "--task" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("missing value for --task");
                    return Ok(2);
                };
                task_id = Some(value.clone());
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
    super::runtime::resource_list(ResourceListParams {
        project_id,
        task_id,
        include_disabled,
    })
}

fn option_value(args: &[String], index: usize, option: &str) -> Option<String> {
    args.get(index + 1).cloned().or_else(|| {
        eprintln!("missing value for {option}");
        None
    })
}

fn parse_kind(value: &str) -> Option<ResourceKind> {
    match value {
        "prompt" => Some(ResourceKind::Prompt),
        "skill" => Some(ResourceKind::Skill),
        "mcp" => Some(ResourceKind::Mcp),
        _ => None,
    }
}

fn parse_scope(value: &str) -> Option<ResourceScope> {
    match value {
        "global" => Some(ResourceScope::Global),
        "project" => Some(ResourceScope::Project),
        "task" => Some(ResourceScope::Task),
        _ => None,
    }
}

fn resource_create(args: &[String]) -> std::io::Result<i32> {
    let mut kind = None;
    let mut name = None;
    let mut description = String::new();
    let mut scope = None;
    let mut project_id = None;
    let mut task_id = None;
    let mut provider = None;
    let mut content = None;
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--kind" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                let Some(value) = parse_kind(&value) else {
                    eprintln!("invalid resource kind: {value}");
                    return Ok(2);
                };
                kind = Some(value);
                index += 2;
            }
            "--name" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                name = Some(value);
                index += 2;
            }
            "--description" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                description = value;
                index += 2;
            }
            "--scope" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                let Some(value) = parse_scope(&value) else {
                    eprintln!("invalid resource scope: {value}");
                    return Ok(2);
                };
                scope = Some(value);
                index += 2;
            }
            "--project" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                project_id = Some(value);
                index += 2;
            }
            "--task" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                task_id = Some(value);
                index += 2;
            }
            "--provider" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                provider = Some(value);
                index += 2;
            }
            "--content" => {
                let Some(value) = option_value(args, index, option) else {
                    return Ok(2);
                };
                content = Some(value);
                index += 2;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    let (Some(kind), Some(name), Some(scope), Some(content)) = (kind, name, scope, content) else {
        eprintln!(
            "usage: herdr resource create --kind prompt|skill|mcp --name NAME --scope global|project|task --content TEXT [OPTIONS]"
        );
        return Ok(2);
    };
    super::runtime::resource_create(ResourceCreateParams {
        kind,
        name,
        description,
        scope,
        project_id,
        task_id,
        provider,
        content,
    })
}

fn resource_update(args: &[String]) -> std::io::Result<i32> {
    let Some(resource_id) = args.first() else {
        eprintln!("usage: herdr resource update <resource_id> [OPTIONS]");
        return Ok(2);
    };
    let mut name = None;
    let mut description = None;
    let mut content = None;
    let mut enabled = None;
    let mut index = 1;
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
            "--description" => {
                description = option_value(args, index, option);
                if description.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--content" => {
                content = option_value(args, index, option);
                if content.is_none() {
                    return Ok(2);
                }
                index += 2;
            }
            "--enable" => {
                enabled = Some(true);
                index += 1;
            }
            "--disable" => {
                enabled = Some(false);
                index += 1;
            }
            other => {
                eprintln!("unknown option: {other}");
                return Ok(2);
            }
        }
    }
    super::runtime::resource_update(ResourceUpdateParams {
        resource_id: resource_id.clone(),
        name,
        description,
        content,
        enabled,
    })
}

fn resource_delete(args: &[String]) -> std::io::Result<i32> {
    if args.len() != 1 {
        eprintln!("usage: herdr resource delete <resource_id>");
        return Ok(2);
    }
    super::runtime::resource_delete(ResourceTarget {
        resource_id: args[0].clone(),
    })
}

fn print_resource_help() {
    println!("herdr resource — manage reusable prompts, skills, and MCP resources");
    println!();
    println!("usage: herdr resource <subcommand>");
    println!("  list [--project ID] [--task ID] [--include-disabled]");
    println!("  create --kind prompt|skill|mcp --name NAME --scope global|project|task --content TEXT [OPTIONS]");
    println!("  update <resource_id> [--name NAME] [--description TEXT] [--content TEXT] [--enable|--disable]");
    println!("  delete <resource_id>");
}
