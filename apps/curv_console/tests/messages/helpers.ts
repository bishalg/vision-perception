/**
 * Shared assertions of the per-file message tests (I18N-15). Each `tests/messages/<file>.test.ts` calls
 * `describeMessages` with its `{ en, ne }` pair and a `NE_PENDING` flag: while `true` (the file's `ne` is still the
 * `// ne: TODO` copy of `en`) only the shape assertions run; the owning wave package sets it to `false`.
 */
import { describe, expect, it } from "vitest";

/** Every string leaf (functions are called with sample arguments), with its path. */
export function leaves(value: unknown, path: string, out: [string, string][] = []): [string, string][] {
  if (typeof value === "string") out.push([path, value]);
  else if (typeof value === "function") {
    try {
      const text = (value as (...a: unknown[]) => unknown)("x", 3, 4, 5, 6);
      if (typeof text === "string") out.push([path, text]);
    } catch {
      // a function that needs a richer argument than the sample is skipped
    }
  } else if (typeof value === "object" && value !== null) {
    for (const [k, v] of Object.entries(value)) leaves(v, `${path}.${k}`, out);
  }
  return out;
}

export function shape(value: unknown): unknown {
  if (typeof value === "function") return `function/${value.length}`;
  if (Array.isArray(value)) return ["array", ...value.map(shape)];
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, shape(v)]));
  }
  return typeof value;
}

const DEVANAGARI = /[ऀ-ॿ]/;
const PLACEHOLDERS = /\{[A-Za-z_]\w*\}|\$\{[^}]*\}/g;

export interface MessageOptions {
  /** True while `ne` is the `// ne: TODO` copy of `en`: skips the Nepali-content assertions. */
  NE_PENDING: boolean;
  /** Paths (`a.b.c`) whose `ne` text may equal the `en` text: brand names, units, codes such as MRN, TOTP, CSV. */
  NE_SAME_AS_EN?: readonly string[];
}

export function describeMessages(name: string, messages: { en: unknown; ne: unknown }, options: MessageOptions): void {
  const same = new Set(options.NE_SAME_AS_EN ?? []);
  describe(`${name} messages`, () => {
    it("en and ne have the same shape (keys, function arity)", () => {
      expect(shape(messages.ne)).toEqual(shape(messages.en));
    });

    describe.skipIf(options.NE_PENDING)("ne content", () => {
      const en = () => leaves(messages.en, name);
      const ne = () => new Map(leaves(messages.ne, name));

      it("has no remaining TODO", () => {
        const neTexts = ne();
        expect(en().filter(([p]) => /\bTODO\b/.test(neTexts.get(p) ?? ""))).toEqual([]);
      });

      it("differs from en except for the listed keys, and contains Devanagari", () => {
        const problems: string[] = [];
        const neTexts = ne();
        for (const [path, enText] of en()) {
          const neText = neTexts.get(path) ?? "";
          const key = path.slice(name.length + 1);
          if (same.has(key)) continue;
          if (neText === enText) problems.push(`${path}: same as en`);
          else if (!DEVANAGARI.test(neText)) problems.push(`${path}: no Devanagari`);
        }
        expect(problems).toEqual([]);
      });

      it("keeps the placeholders", () => {
        const problems: string[] = [];
        const neTexts = ne();
        for (const [path, enText] of en()) {
          const a = [...(enText.match(PLACEHOLDERS) ?? [])].sort().join(",");
          const b = [...((neTexts.get(path) ?? "").match(PLACEHOLDERS) ?? [])].sort().join(",");
          if (a !== b) problems.push(`${path}: ${a} vs ${b}`);
        }
        expect(problems).toEqual([]);
      });
    });
  });
}
