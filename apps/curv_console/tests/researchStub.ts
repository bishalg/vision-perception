/**
 * A stand-in for the Rust core in the research tests: the same envelope shapes as asd-wasm, with trivial arithmetic so
 * a test can see a draft differ from the published profile without the real build. It exports no `reprocessSession`
 * or `paramRegistry` (WP-X.4 has not landed), like the core today.
 */
import { AsdCore, type AsdWasmExports } from "@/lib/asd/core";

const env = (result: unknown) => JSON.stringify({ ok: true, result });

export const calls = { rescore: 0, validate: 0 };

export function stubModule(): AsdWasmExports {
  const exports = {
    contractInfo: () => env({ coreVersion: "stub", sessionSchemaVersions: [], sessionSchemaIds: [], scoringProfileSchemaVersion: "1.2.0", configBundleSchemaId: "x", lowConfidenceValidRatio: 0.6 }),
    defaultScoringProfile: () => env({ schemaVersion: "1.2.0", flag: { greenFloor: 0.65, yellowFloor: 0.35, lowConfidenceShare: 0.5, atypicalShareForRed: 0.4 }, lowConfidenceValidRatio: 0.6, qualityGate: {} }),
    validateScoringProfile: (profileJson: string) => {
      calls.validate += 1;
      const g = (JSON.parse(profileJson) as { flag?: { greenFloor?: unknown } }).flag?.greenFloor;
      const bad = typeof g === "number" && (g > 1 || g < 0);
      const message = "flag.greenFloor must be in [0, 1]";
      return env({ valid: !bad, problems: bad ? [message] : [], fieldProblems: bad ? [{ path: "flag.greenFloor", message }] : [] });
    },
    rescoreSession: (_session: string, profileJson: string) => {
      calls.rescore += 1;
      const floor = (JSON.parse(profileJson) as { flag?: { greenFloor?: number } }).flag?.greenFloor ?? 0.65;
      return env({
        sessionId: "s",
        schemaVersion: "1.8",
        metricSource: "sessionReport",
        score: {
          ageBracketId: "24-35",
          ageMonths: 30,
          compositeScore: floor,
          videos: [{ stimulusId: "stim_01", title: "Clip one", rows: [], subscore: 0.5, validSampleRatio: 0.9, lowConfidence: false, hasBaseline: true, weight: 1 }],
          baselineStatus: "ok",
          baselineSchemaVersion: "1.0.0",
          scoredAtUtc: "2026-11-03T00:00:00Z",
        },
        flag: { flag: floor > 0.9 ? "red" : "green", headline: "h", detail: "d", isScreeningSignalOnly: true },
        device: { compositeScore: null, attentionFlag: null },
        warnings: [],
      });
    },
    lastPanic: () => undefined,
  };
  return exports as unknown as AsdWasmExports;
}

export const stubCore = () => new AsdCore(async () => stubModule());
