const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const packageRoot = path.resolve(__dirname, "..");
const runtimeDirectory = path.join(packageRoot, "bin", "runtime");
const executable = path.join(runtimeDirectory, "herdr.exe");
const releaseAsset = "https://github.com/Bas-Martin/herdr-9000/releases/download/v0.8.2-herdr9000.1/herdr-windows-x86_64.zip";

if (process.platform !== "win32" || process.arch !== "x64") {
  throw new Error("herdr-9000 currently supports Windows x64 only");
}

if (fs.existsSync(executable)) {
  process.exit(0);
}

function quotePowerShell(value) {
  return `'${value.replaceAll("'", "''")}'`;
}

async function install() {
  fs.mkdirSync(runtimeDirectory, { recursive: true });
  const zipPath = path.join(os.tmpdir(), `herdr-9000-${process.pid}.zip`);
  const extractDirectory = path.join(os.tmpdir(), `herdr-9000-${process.pid}`);

  try {
    const response = await fetch(releaseAsset);
    if (!response.ok) {
      throw new Error(`download failed with HTTP ${response.status}`);
    }
    fs.writeFileSync(zipPath, Buffer.from(await response.arrayBuffer()));

    const command = [
      "Expand-Archive",
      "-LiteralPath", quotePowerShell(zipPath),
      "-DestinationPath", quotePowerShell(extractDirectory),
      "-Force",
    ].join(" ");
    const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", command], {
      stdio: "inherit",
      windowsHide: true,
    });
    if (result.status !== 0) {
      throw new Error(`archive extraction failed with exit code ${result.status}`);
    }

    fs.cpSync(extractDirectory, runtimeDirectory, { recursive: true });
    if (!fs.existsSync(executable)) {
      throw new Error("downloaded archive does not contain herdr.exe");
    }
  } finally {
    fs.rmSync(zipPath, { force: true });
    fs.rmSync(extractDirectory, { recursive: true, force: true });
  }
}

install().catch((error) => {
  console.error(`herdr-9000 installation failed: ${error.message}`);
  process.exit(1);
});
