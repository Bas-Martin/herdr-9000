"""Profile the production Windows launcher against a real, isolated VS Code download."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import tomllib
import urllib.request
import zipfile

repo = Path(__file__).resolve().parent.parent


def block(source, declaration):
    match = re.search(r"(?ms)^" + re.escape(declaration) + r".*?^}", source)
    if match is None:
        raise RuntimeError(f"Cannot locate production declaration: {declaration}")
    return match.group()


application_source = (repo / "src/open_with.rs").read_text()
platform_source = (repo / "src/platform/mod.rs").read_text()
launch_source = (repo / "src/platform/open_with.rs").read_text()
application = block(application_source, "pub(crate) enum Application")
application_impl = block(application_source, "impl Application")
quote = block(platform_source, "pub(crate) fn quote_windows_command_line_arg")
launch = block(launch_source, "pub(crate) fn launch")
windows = tomllib.loads((repo / "Cargo.toml").read_text())["target"]["cfg(windows)"]["dependencies"]["windows"]

with tempfile.TemporaryDirectory(prefix="herdr-editor-profile-", ignore_cleanup_errors=True) as directory:
    root = Path(directory)
    (root / "src").mkdir()
    (root / "Cargo.toml").write_text(
        '[package]\nname = "herdr-editor-profile"\nversion = "0.0.0"\nedition = "2021"\n'
        '[dependencies]\nwindows = { version = "=' + windows["version"] + '", features = '
        + json.dumps(windows["features"]) + ' }\n'
    )
    (root / "src/main.rs").write_text(
        "mod open_with {\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n"
        + application + "\n" + application_impl + "\n}\nmod platform {\n" + quote
        + "\npub mod open_with {\nuse std::path::Path;\nuse crate::open_with::Application;\n"
        + launch + "\n}\n}\n" + r'''
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let application = match args[1].to_str().unwrap() {
        "vscode" => open_with::Application::VsCode,
        "visual-studio" => open_with::Application::VisualStudio,
        "explorer" => open_with::Application::Explorer,
        _ => panic!("unknown application"),
    };
    let executable = std::path::Path::new(&args[2]);
    let target = std::path::Path::new(&args[3]);
    let start = std::time::Instant::now();
    let result = platform::open_with::launch(application, Some(executable), target);
    println!("launcher_pid={} elapsed={:?} result={:?}", std::process::id(), start.elapsed(), result);
    if result.is_err() { std::process::exit(1); }
}
'''
    )
    if os.name != "nt":
        subprocess.run(["cargo", "check", "--target", "x86_64-pc-windows-msvc"], cwd=root, check=True)
    else:
        subprocess.run(["cargo", "build"], cwd=root, check=True)
        launcher = root / "target/debug/herdr-editor-profile.exe"
        metadata = json.load(urllib.request.urlopen("https://update.code.visualstudio.com/api/update/win32-x64-archive/stable/latest", timeout=60))
        archive = root / "code.zip"
        urllib.request.urlretrieve(metadata["url"], archive)
        with archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != metadata["sha256hash"]:
            raise RuntimeError("VS Code archive checksum mismatch")
        editor = root / "VS Code"
        with zipfile.ZipFile(archive) as downloaded:
            downloaded.extractall(editor)
        print("VS Code version:", metadata.get("productVersion"), flush=True)
        checkout = root / "workspace ü & launch"
        checkout.mkdir()
        (checkout / "README.txt").write_text("Isolated native editor-launch profile.\n")
        owned_processes = "$p = Get-CimInstance Win32_Process -Filter \"Name='Code.exe'\" | Where-Object { $_.ExecutablePath -eq $env:HERDR_PROFILE_CODE }; "
        os.environ["HERDR_PROFILE_CODE"] = str(editor / "Code.exe")
        try:
            for round_number in (1, 2):
                print(f"OPEN ROUND {round_number}", flush=True)
                subprocess.run([str(launcher), "vscode", str(editor / "Code.exe"), str(checkout)], check=True)
                time.sleep(10)
                subprocess.run(["pwsh", "-NoProfile", "-Command", owned_processes + "$p | ForEach-Object { $parent = Get-CimInstance Win32_Process -Filter ('ProcessId=' + $_.ParentProcessId); [pscustomobject]@{ PID=$_.ProcessId; ParentPID=$_.ParentProcessId; Parent=$parent.Name; WorkingSetMiB=[math]::Round($_.WorkingSetSize/1MB,2); CommandLine=$_.CommandLine } } | ConvertTo-Json -Depth 3"], check=True)
                subprocess.run(["cmd", "/d", "/c", str(editor / "bin/code.cmd"), "--status"], check=True, timeout=60)
        finally:
            subprocess.run(["pwsh", "-NoProfile", "-Command", owned_processes + "$p | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }"], check=False)
