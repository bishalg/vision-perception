"use client";

/**
 * Browser loader of the Rust core: `/asd-wasm/asd_wasm.js` and `asd_wasm_bg.wasm`, copied into `public/` by
 * `scripts/prepare-asd-wasm.mjs`. The import is left to the browser (not bundled) so the build never depends on the
 * wasm output; a missing build only makes the core `unavailable`. One shared instance per page.
 */
import { AsdCore, type AsdWasmExports } from "./core";

export const ASD_WASM_BASE = "/asd-wasm/";

type WasmModule = AsdWasmExports & { default: (init: { module_or_path: string }) => Promise<unknown> };

async function loadBrowser(instance: number): Promise<AsdWasmExports> {
  // A new query string gives a fresh ES module (and so a fresh wasm instance) after a panic.
  const url = `${ASD_WASM_BASE}asd_wasm.js${instance ? `?instance=${instance}` : ""}`;
  const mod = (await import(/* webpackIgnore: true */ /* turbopackIgnore: true */ url)) as WasmModule;
  await mod.default({ module_or_path: `${ASD_WASM_BASE}asd_wasm_bg.wasm` });
  return mod;
}

let shared: AsdCore | undefined;

export function browserCore(): AsdCore {
  return (shared ??= new AsdCore(loadBrowser));
}
