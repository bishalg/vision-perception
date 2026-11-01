/**
 * The real Rust core (asd-wasm `--target web` build) loaded in Node for the unit tests: the same JS glue the browser
 * loads, initialised with the bytes of the `.wasm` file, from
 * `public/asd-wasm` (scripts/prepare-asd-wasm.mjs); tests that need it are skipped when it is missing.
 */
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { AsdCore, type AsdWasmExports } from "@/lib/asd/core";

// Vite only imports files under the console root, so the tests use the copy `npm test` makes (`pretest`).
const dir = resolve(process.cwd(), "public/asd-wasm");

export const WASM_DIR =
  existsSync(join(dir, "asd_wasm.js")) && existsSync(join(dir, "asd_wasm_bg.wasm")) ? dir : undefined;
export const HAS_WASM = Boolean(WASM_DIR);

type Glue = AsdWasmExports & { default: (init: { module_or_path: BufferSource }) => Promise<unknown> };

export function nodeCore(): AsdCore {
  return new AsdCore(async (instance) => {
    if (!WASM_DIR) throw new Error("asd-wasm build not found");
    const url = join(WASM_DIR, "asd_wasm.js") + (instance ? `?instance=${instance}` : "");
    const mod = (await import(/* @vite-ignore */ url)) as Glue;
    await mod.default({ module_or_path: readFileSync(join(WASM_DIR, "asd_wasm_bg.wasm")) });
    return mod;
  });
}
