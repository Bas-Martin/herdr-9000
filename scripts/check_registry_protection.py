import os
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: check_registry_protection.py <executable> <herdr|herdr-dev>")
    executable = Path(sys.argv[1]).resolve()
    app_directory = sys.argv[2]
    if app_directory not in {"herdr", "herdr-dev"}:
        raise ValueError("Expected herdr or herdr-dev as the application directory")

    for filename in ("projects.json", "tasks.json"):
        for failure in ("parse", "read"):
            with tempfile.TemporaryDirectory(prefix="herdr-registry-protection-") as temporary:
                root = Path(temporary)
                environment = {
                    key: value for key, value in os.environ.items() if not key.startswith("HERDR_")
                }
                for variable, directory in {
                    "HOME": "home",
                    "USERPROFILE": "home",
                    "APPDATA": "roaming",
                    "LOCALAPPDATA": "local",
                    "XDG_CONFIG_HOME": "config",
                    "XDG_STATE_HOME": "state",
                }.items():
                    directory_path = root / directory
                    directory_path.mkdir(exist_ok=True)
                    environment[variable] = str(directory_path)

                config = root / "config" / app_directory
                config.mkdir(parents=True)
                registry = config / filename
                original = registry
                if failure == "read":
                    registry.mkdir()
                    original = registry / "original"
                contents = b'{"version":1,"original_data":'
                original.write_bytes(contents)

                result = subprocess.run(
                    [str(executable), "server"],
                    cwd=root,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                if result.returncode == 0 or filename not in result.stdout + result.stderr:
                    raise RuntimeError(
                        f"Startup did not report the {failure} failure for {filename}: "
                        f"{result.stdout}{result.stderr}"
                    )
                if original.read_bytes() != contents:
                    raise RuntimeError(f"Startup modified the unread registry: {filename}")
                print(f"Preserved {filename} after {failure} failure")


if __name__ == "__main__":
    main()
