const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { runtimeHashes, verifyRuntime } = require("../packaging/npm/bin/postinstall.js");

const root = path.resolve(__dirname, "..");
let staging;
try {
  if (process.argv.length !== 4) {
    throw new Error("usage: node scripts/package_npm.js <validated-Windows-stage-dir> <output-dir>");
  }
  const source = path.resolve(process.argv[2]);
  const output = path.resolve(process.argv[3]);
  const metadata = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
  if (!/^[0-9a-f]{40}$/.test(sourceCommit)) throw new Error("git HEAD must be a full commit SHA");
  const binaryVersion = `herdr ${metadata.version.split("-")[0]}-custom.${sourceCommit}`;
  metadata.version += `.git.${sourceCommit}`;
  const files = runtimeHashes(source);
  const conpty = JSON.parse(fs.readFileSync(path.join(root, "packaging/windows/conpty.json"), "utf8"));
  const pinnedFiles = [...conpty.bundles.x86_64.files, ...conpty.notices];
  const expectedNames = ["herdr.exe", "conpty/herdr-conpty.json", ...pinnedFiles.map((file) => file.destination)];
  if (Object.keys(files).length !== expectedNames.length || expectedNames.some((name) => !Object.hasOwn(files, name)) ||
      pinnedFiles.some((file) => files[file.destination] !== file.sha256)) {
    throw new Error("Windows stage does not match the pinned ConPTY bundle");
  }
  const marker = JSON.parse(fs.readFileSync(path.join(source, "conpty/herdr-conpty.json"), "utf8"));
  if (marker.schema_version !== 1 || marker.package !== conpty.package.id || marker.version !== conpty.package.version ||
      marker.architecture !== "x86_64" || Object.keys(marker.files).length !== conpty.bundles.x86_64.files.length ||
      conpty.bundles.x86_64.files.some((file) => marker.files[file.destination] !== file.sha256)) {
    throw new Error("Windows stage ConPTY marker mismatch");
  }
  const manifest = { sourceCommit, binaryVersion, npmPackageVersion: metadata.version, files };
  staging = fs.mkdtempSync(path.join(os.tmpdir(), "herdr-npm-"));
  const bin = path.join(staging, "packaging/npm/bin");
  fs.mkdirSync(bin, { recursive: true });
  for (const name of ["herdr9000.js", "postinstall.js"]) {
    fs.copyFileSync(path.join(root, "packaging/npm/bin", name), path.join(bin, name));
  }
  fs.cpSync(source, path.join(bin, "runtime"), { recursive: true });
  fs.writeFileSync(path.join(bin, "runtime-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  fs.writeFileSync(path.join(staging, "package.json"), JSON.stringify(metadata, null, 2) + "\n");
  fs.copyFileSync(path.join(root, "LICENSE"), path.join(staging, "LICENSE"));
  verifyRuntime(path.join(bin, "runtime"), manifest, metadata.version);
  // npm.cmd is a Windows command script; invoke it through the command processor.
  const packed = JSON.parse(execFileSync(process.platform === "win32" ? "cmd.exe" : "npm",
    process.platform === "win32" ? ["/d", "/s", "/c", "npm pack --ignore-scripts --json"] : ["pack", "--ignore-scripts", "--json"],
    { cwd: staging, encoding: "utf8" }));
  const filename = packed[0].filename;
  if (path.basename(filename) !== filename || !filename.endsWith(".tgz")) throw new Error("invalid npm pack output");
  fs.mkdirSync(output, { recursive: true });
  fs.copyFileSync(path.join(staging, filename), path.join(output, filename));
  console.log(path.join(output, filename));
} catch (error) {
  console.error(`npm packaging failed: ${error.message}`);
  process.exitCode = 1;
} finally {
  if (staging) fs.rmSync(staging, { recursive: true, force: true });
}
