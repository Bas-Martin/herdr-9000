use crate::api::schema::{Method, Request, TmuxPaneCaptureParams, TmuxPaneSendKeysParams};

pub(super) fn run_tmux_command(args: &[String]) -> std::io::Result<i32> {
    let Some(subcommand) = args.first().map(String::as_str) else {
        print_tmux_help();
        return Ok(2);
    };

    match subcommand {
        "list" if args.len() == 1 => super::runtime::tmux_list(),
        "capture" => tmux_capture(&args[1..]),
        "send-keys" => tmux_send_keys(&args[1..]),
        "kill" => tmux_action(&args[1..], true),
        "focus" => tmux_action(&args[1..], false),
        "help" | "--help" | "-h" => {
            print_tmux_help();
            Ok(0)
        }
        _ => {
            print_tmux_help();
            Ok(2)
        }
    }
}

fn tmux_capture(args: &[String]) -> std::io::Result<i32> {
    let Some(pane_id) = args.first() else {
        eprintln!("usage: herdr tmux capture <pane_id> [--lines N]");
        return Ok(2);
    };
    let mut lines = 80;
    let mut index = 1;
    while index < args.len() {
        if args[index] != "--lines" {
            eprintln!("unknown option: {}", args[index]);
            return Ok(2);
        }
        let Some(value) = args.get(index + 1) else {
            eprintln!("missing value for --lines");
            return Ok(2);
        };
        lines = match super::parse_u64_flag("--lines", value) {
            Ok(value) => value.min(u64::from(u32::MAX)) as u32,
            Err(error) => {
                eprintln!("{error}");
                return Ok(2);
            }
        };
        index += 2;
    }
    let response = super::send_request(&Request {
        id: "cli:tmux:capture".into(),
        method: Method::TmuxPaneCapture(TmuxPaneCaptureParams {
            pane_id: pane_id.clone(),
            lines,
        }),
    })?;
    if response.get("error").is_some() {
        return super::print_response(&response);
    }
    if let Some(output) = response["result"]["output"].as_str() {
        crate::platform::begin_cli_output();
        std::print!("{output}");
    } else {
        return super::print_response(&response);
    }
    Ok(0)
}

fn tmux_send_keys(args: &[String]) -> std::io::Result<i32> {
    let Some(pane_id) = args.first() else {
        eprintln!("usage: herdr tmux send-keys <pane_id> <key> [key ...]");
        return Ok(2);
    };
    if args.len() < 2 {
        eprintln!("usage: herdr tmux send-keys <pane_id> <key> [key ...]");
        return Ok(2);
    }
    super::runtime::tmux_send_keys(TmuxPaneSendKeysParams {
        pane_id: pane_id.clone(),
        keys: args[1..].to_vec(),
    })
}

fn tmux_action(args: &[String], kill: bool) -> std::io::Result<i32> {
    let Some(pane_id) = args.first() else {
        eprintln!(
            "usage: herdr tmux {} <pane_id>",
            if kill { "kill" } else { "focus" }
        );
        return Ok(2);
    };
    if args.len() != 1 {
        eprintln!(
            "usage: herdr tmux {} <pane_id>",
            if kill { "kill" } else { "focus" }
        );
        return Ok(2);
    }
    if kill {
        super::runtime::tmux_kill(pane_id.clone())
    } else {
        super::runtime::tmux_focus(pane_id.clone())
    }
}

fn print_tmux_help() {
    eprintln!("herdr tmux commands:");
    eprintln!("  herdr tmux list");
    eprintln!("  herdr tmux capture <pane_id> [--lines N]");
    eprintln!("  herdr tmux send-keys <pane_id> <key> [key ...]");
    eprintln!("  herdr tmux kill <pane_id>");
    eprintln!("  herdr tmux focus <pane_id>");
}
