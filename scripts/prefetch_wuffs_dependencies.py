from __future__ import annotations

import os
import re
import subprocess
import tempfile
from pathlib import Path
from urllib.parse import urlsplit


def main() -> None:
    root = Path(__file__).resolve().parents[1] / "vendor" / "libghostty-vt"
    manifest = (root / "pkg" / "wuffs" / "build.zig.zon").read_text(encoding="utf-8")
    dependencies = re.findall(
        r'\.url\s*=\s*"([^"]+)"\s*,\s*\.hash\s*=\s*"([^"]+)"',
        manifest,
    )
    if not dependencies or len(dependencies) != len(re.findall(r"\.url\s*=", manifest)):
        raise RuntimeError("Unable to read every pinned Wuffs dependency")
    cache = Path(os.environ["ZIG_GLOBAL_CACHE_DIR"]).resolve()
    zig = os.environ.get("ZIG", "zig")
    for url, expected in dependencies:
        parsed = urlsplit(url)
        if parsed.scheme != "https":
            raise RuntimeError(f"Expected an HTTPS dependency URL: {url}")
        if (cache / "p" / f"{expected}.tar.gz").is_file():
            continue
        print(f"Prefetching {url}", flush=True)
        with tempfile.TemporaryDirectory(prefix="herdr-wuffs-") as directory:
            archive = Path(directory) / Path(parsed.path).name
            subprocess.run(
                [
                    "curl", "--proto", "=https", "--fail", "--location",
                    "--silent", "--show-error", "--max-time", "120",
                    "--output", str(archive), url,
                ],
                check=True,
            )
            actual = subprocess.check_output(
                [zig, "fetch", "--global-cache-dir", str(cache), str(archive)],
                cwd=root,
                text=True,
            ).strip()
            if actual != expected:
                raise RuntimeError(
                    f"Dependency hash mismatch for {url}: expected {expected}, got {actual}"
                )


if __name__ == "__main__":
    main()
