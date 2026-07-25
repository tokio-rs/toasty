import { spawn } from "node:child_process";

const address = "http://127.0.0.1:8791";
const wrangler = spawn(
  "npx",
  ["--yes", "wrangler@4.114.0", "dev", "--local", "--port", "8791"],
  { cwd: import.meta.dirname, stdio: ["ignore", "pipe", "pipe"] },
);

let output = "";
for (const stream of [wrangler.stdout, wrangler.stderr]) {
  stream.on("data", (chunk) => {
    output += chunk.toString();
    process.stderr.write(chunk);
  });
}

async function waitUntilReady() {
  for (let attempt = 0; attempt < 600; attempt += 1) {
    if (wrangler.exitCode !== null) {
      throw new Error(`Wrangler exited before becoming ready\n${output}`);
    }
    try {
      const response = await fetch(`${address}/tests`);
      if (response.ok) return response.json();
    } catch {
      // The Worker is still building.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for Wrangler\n${output}`);
}

async function stopWrangler() {
  if (wrangler.exitCode !== null) return;
  wrangler.kill("SIGTERM");
  await new Promise((resolve) => wrangler.once("exit", resolve));
}

async function main() {
  const manifest = await waitUntilReady();
  const tests = manifest.tests;
  console.log(`compatibility date: ${manifest.compatibility_date}`);
  let failed = 0;
  for (const name of tests) {
    const response = await fetch(
      `${address}/run?name=${encodeURIComponent(name)}`,
    );
    if (!response.ok) {
      failed += 1;
      console.error(`FAILED ${name}: HTTP ${response.status}`);
      continue;
    }
    const result = await response.json();
    if (result.status === "passed") {
      console.log(`ok ${name}`);
    } else {
      failed += 1;
      console.error(`FAILED ${name}: ${result.error}`);
      console.error(result.operations);
    }
  }
  console.log(`${tests.length - failed} passed; ${failed} failed`);
  if (failed !== 0) process.exitCode = 1;
}

try {
  await main();
} finally {
  await stopWrangler();
}
