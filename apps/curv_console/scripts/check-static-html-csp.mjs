#!/usr/bin/env node
/**
 * CSP-1 post-build guard (npm `postbuild`). The console's CSP is nonce based with `'strict-dynamic'`, so a page
 * prerendered to static HTML at build time ships scripts with no nonce, which the browser blocks.
 *
 * Every page renders per request (`connection()` in the root layout). The one exception Next forces is the synthetic
 * `/_global-error` document (Next's built-in error page, also copied to `pages/500.html` and served as the
 * last-resort 500 page; root-layout errors during a normal render are rendered per request, with the nonce). This
 * script strips its scripts and script preloads: the page is static HTML whose "Reload" button is a plain form, so it
 * works without JavaScript, and the browser no longer reports blocked scripts on it.
 *
 * Then it checks every route in `.next/prerender-manifest.json` (plus `pages/*.html`): any static HTML that still
 * contains a `<script>` or script preload without a nonce fails the build.
 */
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const consoleDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const distDir = process.argv[2] ? resolve(process.argv[2]) : join(consoleDir, ".next");
const serverDir = join(distDir, "server");

/** Next always emits this route as static; it is stripped instead of failing the build. */
const FORCED_STATIC = new Set(["/_global-error"]);

const SCRIPT_RE = /<script\b[^>]*>[\s\S]*?<\/script>/gi;

function scriptPreloads(html) {
  return [...html.matchAll(/<link\b[^>]*>/gi)]
    .map((m) => m[0])
    .filter((tag) => /\brel="modulepreload"/i.test(tag) || (/\brel="preload"/i.test(tag) && /\bas="script"/i.test(tag)));
}

export function stripScripts(html) {
  let out = html.replace(SCRIPT_RE, "");
  for (const tag of scriptPreloads(out)) out = out.replace(tag, "");
  return out;
}

/** Script tags and script preloads in `html` that carry no nonce attribute. */
export function unNoncedScripts(html) {
  const tags = [...html.matchAll(/<script\b[^>]*>/gi)].map((m) => m[0]).concat(scriptPreloads(html));
  return tags.filter((tag) => !/\snonce="[^"]+"/i.test(tag));
}

function staticHtmlFiles() {
  const files = [];
  const manifestPath = join(distDir, "prerender-manifest.json");
  if (!existsSync(manifestPath)) throw new Error(`missing ${manifestPath}: run next build first`);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  for (const route of Object.keys(manifest.routes ?? {})) {
    const file = join(serverDir, "app", `${route === "/" ? "/index" : route}.html`);
    if (existsSync(file)) files.push({ route, file });
  }
  const pagesDir = join(serverDir, "pages");
  if (existsSync(pagesDir)) {
    for (const name of readdirSync(pagesDir)) {
      // pages/500.html is a copy of the /_global-error document.
      if (name.endsWith(".html")) files.push({ route: name === "500.html" ? "/_global-error" : `/${name}`, file: join(pagesDir, name) });
    }
  }
  return files;
}

function main() {
  const failures = [];
  const files = staticHtmlFiles();
  for (const { route, file } of files) {
    let html = readFileSync(file, "utf8");
    if (FORCED_STATIC.has(route)) {
      const stripped = stripScripts(html);
      if (stripped !== html) writeFileSync(file, stripped);
      html = stripped;
    }
    const bad = unNoncedScripts(html);
    if (bad.length) failures.push(`${route} (${file}): ${bad.length} un-nonced script(s), e.g. ${bad[0]}`);
  }
  if (failures.length) {
    console.error("CSP: static HTML with un-nonced scripts (make the route render per request):\n" + failures.join("\n"));
    process.exit(1);
  }
  console.log(`CSP: ${files.length} static HTML file(s) checked, none carries an un-nonced script.`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
