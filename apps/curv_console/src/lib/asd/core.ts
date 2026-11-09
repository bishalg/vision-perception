/**
 * The Rust core in the console (WP-C.5 over WP-R.6 `asd-wasm`): a typed wrapper over the wasm-bindgen module.
 *
 * Every export of the module takes JSON strings and returns one JSON envelope, `{ok:true,result}` or
 * `{ok:false,error:{kind,message}}`; nothing here scores, validates or decides anything, it only moves JSON across
 * the boundary. Two failures the module cannot report itself become envelopes here:
 * - `unavailable`: the module could not be loaded (not built, network, old browser);
 * - `panic`: wasm32 aborts on a Rust panic (a thrown `RuntimeError`). The message comes from `lastPanic()` and the
 *   instance is dropped, so the next call loads a fresh one (a panicked instance may hold broken state).
 *
 * Environment-agnostic: the browser loader (`./browser`) imports `/asd-wasm/asd_wasm.js`; the unit tests load the
 * same file from `public/asd-wasm` with the bytes of the `.wasm` file.
 */

/** The wasm-bindgen exports the console uses (see `build/asd/wasm/asd_wasm.d.ts`). */
export interface AsdWasmExports {
  contractInfo(): string;
  defaultScoringProfile(): string;
  validateScoringProfile(profileJson: string): string;
  validateCatalog(catalogJson: string): string;
  validateBaselines(baselinesJson: string, requiredIdsJson?: string | null): string;
  rescoreSession(
    sessionJson: string,
    profileJson: string,
    baselinesJson: string,
    optionsJson: string,
    framesJson?: string | null,
  ): string;
  rescoreApiSession(
    sessionJson: string,
    clipsJson: string,
    profileJson: string,
    baselinesJson: string,
    optionsJson: string,
  ): string;
  /** WP-X.4 (RPX-4); absent from a core built before it. */
  paramRegistry?(): string;
  reprocessSession?(
    frameLogJsonl: string,
    catalogJson: string,
    profileJson: string,
    baselinesJson: string,
    optionsJson: string,
  ): string;
  /** WP-X.19 (ADR-0015 V-1..V-5): the replay exports; absent from a core built before it. */
  replayEvents?(framesJson: string, profileJson: string): string;
  replayRibbon?(framesJson: string, clipJson: string, profileJson?: string | null): string;
  replayTrace?(
    framesJson: string,
    clipJson: string,
    baselinesJson: string,
    profileJson: string,
    metricJson: string,
    windowSec: number,
    stepSec: number,
    ageMonths?: number | null,
  ): string;
  expectedBands?(baselinesJson: string, clipId: string, ageMonths: number, biomarker: string, targetAoi?: string | null): string;
  replayHeatmap?(
    framesJson: string,
    clipId: string | null | undefined,
    w: number,
    h: number,
    bandwidthNorm: number,
    fromSec?: number | null,
    toSec?: number | null,
  ): string;
  heatmapCohort?(gridsJson: string, minN?: number | null): string;
  heatmapDifference?(aJson: string, bJson: string): string;
  lastPanic(): string | undefined;
}

/** Names of the exports a caller may invoke through {@link AsdCore.call}. */
export type AsdExportName = Exclude<keyof AsdWasmExports, "lastPanic">;

export type CoreErrorKind = "invalidJson" | "invalidArgument" | "unsupportedSchemaVersion" | "panic" | "unavailable";

export type Envelope<T> = { ok: true; result: T } | { ok: false; error: { kind: CoreErrorKind | string; message: string } };

/** Loads and initialises a fresh module instance; `instance` increases after every panic (use it to bust caches). */
export type AsdLoader = (instance: number) => Promise<AsdWasmExports>;

function isEnvelope(v: unknown): v is Envelope<unknown> {
  return typeof v === "object" && v !== null && typeof (v as { ok?: unknown }).ok === "boolean";
}

export class AsdCore {
  private module: Promise<AsdWasmExports> | null = null;
  private instance = 0;

  constructor(private readonly loader: AsdLoader) {}

  private load(): Promise<AsdWasmExports> {
    if (!this.module) {
      const n = this.instance;
      this.module = this.loader(n).catch((err: unknown) => {
        // A later call retries the load.
        this.module = null;
        throw err;
      });
    }
    return this.module;
  }

  /** Whether the module loads (it is then kept for the next call). */
  async available(): Promise<boolean> {
    try {
      await this.load();
      return true;
    } catch {
      return false;
    }
  }

  /** Whether the loaded module has the export `name` (false when the module does not load). */
  async supports(name: AsdExportName): Promise<boolean> {
    try {
      return typeof (await this.load())[name] === "function";
    } catch {
      return false;
    }
  }

  /** Calls an export and decodes its envelope. Never throws. */
  async call<T>(name: AsdExportName, ...args: (string | number | null | undefined)[]): Promise<Envelope<T>> {
    let mod: AsdWasmExports;
    try {
      mod = await this.load();
    } catch (err) {
      return { ok: false, error: { kind: "unavailable", message: err instanceof Error ? err.message : String(err) } };
    }
    if (typeof mod[name] !== "function") {
      return { ok: false, error: { kind: "unavailable", message: `${name}: this Rust core build does not export it` } };
    }
    let raw: string;
    try {
      raw = (mod[name] as (...a: unknown[]) => string)(...args);
    } catch (err) {
      let message: string | undefined;
      try {
        message = mod.lastPanic() ?? undefined;
      } catch {
        message = undefined;
      }
      this.module = null;
      this.instance += 1;
      return {
        ok: false,
        error: { kind: "panic", message: message ?? (err instanceof Error ? err.message : String(err)) },
      };
    }
    try {
      const parsed: unknown = JSON.parse(raw);
      if (isEnvelope(parsed)) return parsed as Envelope<T>;
    } catch {
      // fall through
    }
    return { ok: false, error: { kind: "invalidJson", message: `${name}: the module returned no envelope` } };
  }
}
