const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { spawnSync } = require("node:child_process");

function runtimeHashes(directory, prefix = "") {
  const hashes = Object.create(null);
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const name = prefix + entry.name;
    if (!/^[A-Za-z0-9_.\/-]+$/.test(name)) {
      throw new Error(`invalid runtime path: ${name}`);
    }
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      Object.assign(hashes, runtimeHashes(file, `${name}/`));
    } else if (entry.isFile()) {
      hashes[name] = createHash("sha256").update(fs.readFileSync(file)).digest("hex");
    } else {
      throw new Error(`runtime must contain only regular files: ${name}`);
    }
  }
  return hashes;
}

function verifyRuntime(directory, manifest, packageVersion) {
  if (process.platform !== "win32" || process.arch !== "x64") {
    throw new Error("herdr-9000 currently supports Windows x64 only");
  }
  const baseVersion = packageVersion.split("-")[0];
  if (!/^[0-9a-f]{40}$/.test(manifest.sourceCommit) ||
      manifest.npmPackageVersion !== packageVersion ||
      !packageVersion.endsWith(`.git.${manifest.sourceCommit}`) ||
      manifest.binaryVersion !== `herdr ${baseVersion}-custom.${manifest.sourceCommit}`) {
    throw new Error("bundled runtime build identity mismatch");
  }
  const actual = runtimeHashes(directory);
  const expected = manifest.files;
  const required = ["herdr.exe", "conpty/herdr-conpty.json", "conpty/conpty.dll",
    "conpty/x64/OpenConsole.exe", "conpty/arm64/OpenConsole.exe"];
  if (!expected || required.some((name) => !Object.hasOwn(expected, name)) ||
      Object.keys(actual).length !== Object.keys(expected).length ||
      Object.entries(actual).some(([name, hash]) => expected[name] !== hash)) {
    throw new Error("bundled runtime SHA-256 mismatch (missing, changed, or unexpected file)");
  }
  // Hash every file before allowing any native code to execute.
  const result = spawnSync(path.join(directory, "herdr.exe"), ["--version"], {
    encoding: "utf8", windowsHide: true, timeout: 30000,
  });
  if (result.error || result.status !== 0 || result.stdout.trim() !== manifest.binaryVersion) {
    throw new Error("bundled runtime --version does not match expected build identity");
  }
}

if (require.main === module) {
  try {
    const metadata = require("../../../package.json");
    const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, "runtime-manifest.json"), "utf8"));
    verifyRuntime(path.join(__dirname, "runtime"), manifest, metadata.version);
    const detection = spawnSync(path.join(__dirname, "runtime", "herdr.exe"), ["--detect-open-with"], {
      encoding: "utf8", windowsHide: true, timeout: 30000,
    });
    if (detection.error || detection.status !== 0) {
      console.warn("Open With detection failed. Configure executable paths in Settings -> Open With.");
    }
  } catch (error) {
    console.error(`herdr-9000 installation failed: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = { runtimeHashes, verifyRuntime };
