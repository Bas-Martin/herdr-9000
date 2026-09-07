#!/usr/bin/env node

const path = require("node:path");
const { spawn } = require("node:child_process");

const executable = path.join(__dirname, "..", "bin", "runtime", "herdr.exe");
const child = spawn(executable, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

child.on("error", (error) => {
  console.error(`herdr9000 failed to start: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  process.exit(code ?? (signal ? 1 : 0));
});
