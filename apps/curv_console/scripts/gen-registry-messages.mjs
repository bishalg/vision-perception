#!/usr/bin/env node
/**
 * Keeps the parameter-registry label keys of `src/messages/research.ts` in step with the Rust registry
 * (`docs/spec/schemas/asd/param-registry-v1.json`, PRM-5, WP-X.0).
 *
 * The file has one generated block per locale between `// <registry:LOCALE:begin>` and `// <registry:LOCALE:end>`:
 * one line `"<id>": { label: "...", help: "..." },` per registry row, in registry order. Running the script adds a
 * line (values "TODO") for a new id, drops the line of a removed id and keeps every written value, so translators
 * and WP-X.5 edit the values in place and re-run it after a registry change.
 *
 *   node scripts/gen-registry-messages.mjs           rewrite the blocks
 *   node scripts/gen-registry-messages.mjs --check   exit 1 when a block is out of step (no write)
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const consoleDir = resolve(here, "..");
const registryPath = resolve(consoleDir, "../../docs/spec/schemas/asd/param-registry-v1.json");
const messagesPath = join(consoleDir, "src/messages/research.ts");
const LOCALES = ["en", "ne"];
const TODO = "TODO";
const LINE = /^\s*("(?:[^"\\]|\\.)*"): \{ label: ("(?:[^"\\]|\\.)*"), help: ("(?:[^"\\]|\\.)*") \},\s*$/;

/** The block for `locale` with existing values kept. */
export function renderBlock(ids, existing) {
  return ids
    .map((id) => {
      const prev = existing.get(id);
      const label = JSON.stringify(prev?.label ?? TODO);
      const help = JSON.stringify(prev?.help ?? TODO);
      return `    ${JSON.stringify(id)}: { label: ${label}, help: ${help} },`;
    })
    .join("\n");
}

/** Updates every locale block of `source`; throws when a block is missing. */
export function updateMessages(source, ids) {
  let out = source;
  for (const locale of LOCALES) {
    const begin = `// <registry:${locale}:begin>`;
    const end = `// <registry:${locale}:end>`;
    const a = out.indexOf(begin);
    const b = out.indexOf(end);
    if (a < 0 || b < a) throw new Error(`${messagesPath}: missing ${begin} ... ${end}`);
    const head = out.slice(0, a + begin.length);
    const body = out.slice(a + begin.length, b);
    const tail = out.slice(b);
    const existing = new Map();
    for (const line of body.split("\n")) {
      const m = LINE.exec(line);
      if (m) existing.set(JSON.parse(m[1]), { label: JSON.parse(m[2]), help: JSON.parse(m[3]) });
    }
    out = `${head}\n${renderBlock(ids, existing)}\n    ${tail}`;
  }
  return out;
}

function main() {
  const registry = JSON.parse(readFileSync(registryPath, "utf8"));
  const ids = registry.params.map((p) => p.id);
  const source = readFileSync(messagesPath, "utf8");
  const next = updateMessages(source, ids);
  if (process.argv.includes("--check")) {
    if (next !== source) {
      console.error(`${messagesPath} is out of step with param-registry-v1.json: run node scripts/gen-registry-messages.mjs`);
      process.exit(1);
    }
    console.log(`OK: ${ids.length} registry ids in every locale`);
    return;
  }
  if (next !== source) writeFileSync(messagesPath, next);
  console.log(`${next === source ? "unchanged" : "wrote"} ${messagesPath} (${ids.length} ids x ${LOCALES.length} locales)`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
