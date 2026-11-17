#!/usr/bin/env python3
"""Profile real OMP restores in disposable Herdr/HOME directories; send no prompts."""

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import time


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", type=Path)
parser.add_argument("omp_session", type=Path, help="Saved OMP JSONL; only copies are opened")
parser.add_argument("--agents", type=int, default=10)
parser.add_argument("--output-seconds", type=float, default=5)
args = parser.parse_args()
if args.agents < 1 or args.output_seconds <= 0:
    parser.error("agents and output-seconds must be positive")
binary = str(args.binary.resolve(strict=True))
source = args.omp_session.resolve(strict=True)
if not source.is_file() or shutil.which("omp") is None:
    parser.error("provide a saved session file and put omp on PATH")

with tempfile.TemporaryDirectory(prefix="herdr-restore-profile-") as directory:
    root = Path(directory)
    config = root / "config" / "herdr"
    home = root / "home"
    runtime = root / "run"
    for path in (config, home, runtime):
        path.mkdir(parents=True)
    config.joinpath("config.toml").write_text(
        'onboarding = false\n[terminal]\ndefault_shell = "/bin/sh"\n'
        'shell_mode = "non_login"\n[session]\nstartup_per_agent_delay_ms = 100\n'
    )
    tabs = []
    for index in range(1, args.agents + 2):
        pane = {"cwd": str(root)}
        if index > 1:
            saved = root / f"agent-{index}.jsonl"
            shutil.copyfile(source, saved)
            pane["agent_session"] = {
                "source": "herdr:omp", "agent": "omp", "kind": "path", "value": str(saved)
            }
        tabs.append({"layout": {"Pane": index}, "panes": {str(index): pane},
                     "zoomed": False, "focused": index, "root_pane": index})
    config.joinpath("session.json").write_text(json.dumps({
        "version": 3, "workspaces": [{"id": "w1", "identity_cwd": str(root),
                                     "tabs": tabs, "active_tab": args.agents}],
        "active": 0, "selected": 0,
    }))
    # Do not inherit credentials, hooks, socket overrides or the user's OMP config.
    environment = {
        "PATH": os.environ["PATH"], "HOME": str(home), "SHELL": "/bin/sh",
        "TERM": "xterm-256color", "LANG": "C.UTF-8", "OMP_SKIP_SETUP": "1",
        "XDG_CONFIG_HOME": str(root / "config"), "XDG_RUNTIME_DIR": str(runtime),
        "HERDR_DISABLE_SOUND": "1",
    }

    def cli(*arguments):
        return subprocess.run([binary, *arguments], env=environment, cwd=root,
                              capture_output=True, text=True, timeout=15)

    with root.joinpath("server-output.log").open("w") as output:
        server = subprocess.Popen([binary, "server"], env=environment, cwd=root,
                                  stdin=subprocess.DEVNULL, stdout=output, stderr=output)
        try:
            deadline = time.monotonic() + 15
            while True:
                response = cli("pane", "list")
                if response.returncode == 0:
                    panes = json.loads(response.stdout)["result"]["panes"]
                    break
                if server.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f"isolated server did not become ready: {response.stderr}")
                time.sleep(0.02)
            producer = (
                "import time,json,pathlib; start=time.time(); end=time.monotonic()+" + str(args.output_seconds)
                + "\nwhile time.monotonic()<end: print('restore-load',flush=True); time.sleep(.01)"
                + "\npathlib.Path(" + repr(str(root / "producer.json"))
                + ").write_text(json.dumps([start,time.time()]))"
            )
            response = cli("pane", "run", panes[0]["pane_id"],
                           "python3 -u -c " + shlex.quote(producer))
            response.check_returncode()
            time.sleep(args.output_seconds + args.agents * 0.1 + 5)
            lines = config.joinpath("herdr-server.log").read_text().splitlines()
            started = next(datetime.fromisoformat(line.split()[0]) for line in lines
                           if 'event="app.startup"' in line)
            launches = []
            for line in lines:
                match = re.search(r'event="pane.spawn.start".*pane_id=(\d+) ', line)
                if match and int(match[1]) > 1:
                    launches.append((datetime.fromisoformat(line.split()[0]) - started).total_seconds())
            detected = {re.search(r"pane=(\d+)", line)[1] for line in lines
                        if "agent=Some(Omp)" in line}
            print(json.dumps({"agents": args.agents, "launch_seconds": launches,
                              "detected_omp": len(detected),
                              "output_seconds": args.output_seconds}, indent=2))
            if len(launches) != args.agents or len(detected) != args.agents:
                raise RuntimeError("not all saved OMP sessions were launched and detected")
            output_start, output_end = json.loads(root.joinpath("producer.json").read_text())
            if not output_start < started.timestamp() + max(launches) < output_end:
                raise RuntimeError("restoration did not finish while the output producer was active")
        finally:
            try:
                stopped = cli("server", "stop")
                if stopped.returncode != 0 and server.poll() is None:
                    server.terminate()
            except (OSError, subprocess.TimeoutExpired):
                if server.poll() is None:
                    server.terminate()
            try:
                server.wait(timeout=15)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
