#!/usr/bin/env node
/**
 * Puts the Rust core's WebAssembly build (rust/crates/asd-wasm, WP-R.6) where the browser loads it for the WP-C.5
 * sandbox re-score and the scoring-profile / baselines validation: `public/asd-wasm/` (git-ignored, generated).
 *
 * Runs before `next dev` and `next build` (npm `predev` / `prebuild`) and before the Playwright dev server.
 *
 * Source: `$ASD_WASM_OUT`, else `<repo>/build/asd/wasm` (the output of `scripts/build_asd_wasm.sh`). When it is
 * missing and `wasm-pack` is on PATH, the script runs that build first. Without either, it warns and leaves the
 * console without the core: pages still build and render, and the sandbox and the editors' Rust validation show
 * "Rust core unavailable" (mock mode and the unit tests keep working). `ASD_WASM_REQUIRED=1` turns the warning into
 * an error (release builds).
 */
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const consoleDir = resolve(here, "..");
const repo = resolve(consoleDir, "../..");
const source = process.env.ASD_WASM_OUT ? resolve(process.env.ASD_WASM_OUT) : join(repo, "build/asd/wasm");
const target = join(consoleDir, "public/asd-wasm");
const FILES = ["asd_wasm.js", "asd_wasm_bg.wasm"];

const complete = (dir) => FILES.every((f) => existsSync(join(dir, f)));

function hasWasmPack() {
  return spawnSync("wasm-pack", ["--version"], { stdio: "ignore" }).status === 0;
}

if (!complete(source) && hasWasmPack()) {
  console.log(`asd-wasm: ${source} is missing; building it (scripts/build_asd_wasm.sh)`);
  const r = spawnSync(join(repo, "scripts/build_asd_wasm.sh"), [], {
    stdio: "inherit",
    env: { ...process.env, ASD_WASM_OUT: source },
  });
  if (r.status !== 0) console.warn("asd-wasm: the build failed");
}

if (!complete(source)) {
  const message =
    `asd-wasm: no build at ${source}. Run scripts/build_asd_wasm.sh (needs wasm-pack) to enable the sandbox ` +
    "re-score and Rust validation in the console.";
  if (process.env.ASD_WASM_REQUIRED === "1") {
    console.error(message);
    process.exit(1);
  }
  console.warn(message);
  process.exit(0);
}

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
for (const f of FILES) copyFileSync(join(source, f), join(target, f));
console.log(`asd-wasm: copied ${FILES.join(", ")} to public/asd-wasm`);
