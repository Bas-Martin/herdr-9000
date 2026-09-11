const fs = require("node:fs");
const crypto = require("node:crypto");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const packageRoot = path.resolve(__dirname, "..");
const runtimeDirectory = path.join(packageRoot, "bin", "runtime");
const executable = path.join(runtimeDirectory, "herdr.exe");
const manifestUrl = "https://raw.githubusercontent.com/Bas-Martin/herdr-9000/main/distribution/latest.json";
const releaseUrlPrefix = "https://github.com/Bas-Martin/herdr-9000/releases/download/";
const bootstrapRelease = {
  url: "https://github.com/Bas-Martin/herdr-9000/releases/download/v0.8.2-herdr9000.1/herdr-windows-x86_64.zip",
  sha256: "8938a7acb67b622ea78e8385420ab47684b05fa41e7db77281521060fc17f14f",
};

if (process.platform !== "win32" || process.arch !== "x64") {
  throw new Error("herdr-9000 currently supports Windows x64 only");
}

function quotePowerShell(value) {
  return `'${value.replaceAll("'", "''")}'`;
}

async function resolveReleaseAsset() {
  try {
    const response = await fetch(manifestUrl);
    if (response.ok) {
      const manifest = await response.json();
      const rawAsset = manifest?.assets?.["windows-x86_64"];
      const url = typeof rawAsset === "string" ? rawAsset : rawAsset?.url;
      const sha256 = manifest?.sha256?.["windows-x86_64"] ?? rawAsset?.sha256;
      if (
        typeof url === "string" &&
        url.startsWith(releaseUrlPrefix) &&
        typeof sha256 === "string" &&
        /^[a-f0-9]{64}$/i.test(sha256)
      ) {
        return { url, sha256: sha256.toLowerCase() };
      }
    }
  } catch {
    // Keep the bootstrap release usable while the manifest is unavailable.
  }

  return bootstrapRelease;
}

async function install() {
  fs.mkdirSync(runtimeDirectory, { recursive: true });
  const zipPath = path.join(os.tmpdir(), `herdr-9000-${process.pid}.zip`);
  const extractDirectory = path.join(os.tmpdir(), `herdr-9000-${process.pid}`);
  const release = await resolveReleaseAsset();

  try {
    const response = await fetch(release.url);
    if (!response.ok) {
      throw new Error(`download failed with HTTP ${response.status}`);
    }
    const archive = Buffer.from(await response.arrayBuffer());
    const digest = crypto.createHash("sha256").update(archive).digest("hex");
    if (digest !== release.sha256) {
      throw new Error(`download checksum mismatch: expected ${release.sha256}, got ${digest}`);
    }
    fs.writeFileSync(zipPath, archive);

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
