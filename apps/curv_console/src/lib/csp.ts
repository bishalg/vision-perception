/**
 * Content-Security-Policy builder (CSP-1). Called per request from `proxy.ts` with a fresh nonce.
 *
 * `'strict-dynamic'` lets the nonced Next scripts load further scripts (the `asd_wasm.js` dynamic import);
 * `'wasm-unsafe-eval'` lets it instantiate the wasm. `style-src` keeps `'unsafe-inline'` because React `style`
 * attributes (charts) cannot carry a nonce; scripts are the injection risk that matters. `'unsafe-eval'` is for
 * `next dev` only (React debug stacks).
 */
export function newNonce(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(16))).toString("base64");
}

export function buildCsp(nonce: string, development: boolean): string {
  const scriptSrc = ["'self'", `'nonce-${nonce}'`, "'strict-dynamic'", "'wasm-unsafe-eval'"];
  if (development) scriptSrc.push("'unsafe-eval'");
  return [
    "default-src 'self'",
    `script-src ${scriptSrc.join(" ")}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' blob:",
    "font-src 'self'",
    "connect-src 'self'",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'none'",
  ].join("; ");
}

/** CSP-2: HSTS, production only (a dev server on http://localhost must not pin the host to HTTPS). */
export const HSTS = "max-age=63072000; includeSubDomains";

export function sendHsts(): boolean {
  return process.env.NODE_ENV === "production";
}
