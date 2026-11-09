/**
 * Display helpers of the dataset pages (WP-X.11). The dataset, its counts and its content hash come from the Rust
 * backend (`asd_contract::datasets`); these only shape them for rendering (no split or hash is computed here).
 */
import type { Dataset, SplitStratumCount } from "@/lib/api/research";

/** Characters of a hash shown before the ellipsis; the full hash stays one click away. */
export const SHORT_HASH_CHARS = 12;

/** `abcdef012345…` for a long hex hash; a short or empty value is returned as is. */
export function shortHash(hash: string, chars = SHORT_HASH_CHARS): string {
  return hash.length > chars ? `${hash.slice(0, chars)}…` : hash;
}

/** One row of the split table: a stratum, or the `total` row. */
export interface SplitRow {
  stratum: string;
  total: boolean;
  train: number;
  validation: number;
  holdout: number;
  all: number;
}

/**
 * The split table of a dataset: one row per stratum as the server counted it (strata in the server's order), then a
 * total row. Empty when the dataset has no split.
 */
export function splitRows(dataset: Pick<Dataset, "split" | "split_counts">): SplitRow[] {
  if (!dataset.split) return [];
  const counts: SplitStratumCount[] = dataset.split_counts ?? [];
  const rows: SplitRow[] = counts.map((c) => ({
    stratum: c.stratum,
    total: false,
    train: c.train,
    validation: c.validation,
    holdout: c.holdout,
    all: c.train + c.validation + c.holdout,
  }));
  const sum = (k: "train" | "validation" | "holdout") => rows.reduce((s, r) => s + r[k], 0);
  const train = sum("train");
  const validation = sum("validation");
  const holdout = sum("holdout");
  rows.push({ stratum: "", total: true, train, validation, holdout, all: train + validation + holdout });
  return rows;
}

/** The server's stratum name for members without an age band (`asd_contract::datasets::UNBANDED`). */
export const UNBANDED = "unbanded";

/** A share as written in the split facts (`0.7`), never rounded away. */
export function formatShare(x: number): string {
  return String(x);
}

/** The rule kinds the backend applies today (the others are refused until WP-X.22). */
export const APPLIED_RULE_KINDS = ["quality_gates", "explicit_list"] as const;
