/**
 * The only module that imports `zod` directly (ESLint `no-restricted-imports` enforces it). Every schema module
 * imports `z` from here, so `jitless` is set before any schema is constructed or parsed, on the server and in every
 * client bundle.
 *
 * Why: zod 4 compiles object validators with `new Function` unless `jitless` is set, and probes `Function("")` to find
 * out whether it may. Under the console CSP (no `'unsafe-eval'` in production) that probe is a reported
 * `securitypolicyviolation`, even though zod falls back. Setting the flag here, in module order, guarantees it is in
 * place first; a component in the root layout would not.
 */
import { z } from "zod";

z.config({ jitless: true });

export * from "zod";
export { z };
