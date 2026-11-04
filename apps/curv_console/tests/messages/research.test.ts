/**
 * ADR-0013 principle 2 and PRM-5: every registry knob has a label and a help text in both locales, no value is left as
 * the generator's "TODO", and `en` and `ne` have the same shape.
 */
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { PARAM_UNITS, REPROCESS_LEVELS } from "@/lib/api/research";
import { paramText } from "@/lib/research/paramText";
import { groupRegistry, paramRegistry } from "@/lib/research/registry";
import { researchMessages } from "@/messages/research";

const LOCALES = ["en", "ne"] as const;

/** Every string leaf (functions are called with sample arguments), with its path. */
function leaves(value: unknown, path: string, out: [string, string][] = []): [string, string][] {
  if (typeof value === "string") out.push([path, value]);
  else if (typeof value === "function") {
    const text = (value as (...a: unknown[]) => unknown)("x", 3, 4, 5, 6);
    if (typeof text === "string") out.push([path, text]);
  } else if (typeof value === "object" && value !== null) {
    for (const [k, v] of Object.entries(value)) leaves(v, `${path}.${k}`, out);
  }
  return out;
}

function shape(value: unknown): unknown {
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, shape(v)]));
  }
  return typeof value;
}

describe("research messages", () => {
  for (const locale of LOCALES) {
    describe(locale, () => {
      const m = researchMessages[locale];

      it("has a label and a help text for every registry row in the domain catalogue (PRM-5)", () => {
        for (const p of paramRegistry.params) {
          const t = paramText(locale, p);
          expect(t.label.trim(), `${locale} label of ${p.id}`).not.toBe("");
          expect(t.label, `${locale} label of ${p.id} resolves`).not.toBe(p.id);
          expect(t.help, `${locale} help of ${p.id} resolves`).not.toBe(p.id);
        }
      });

      it("fails on a remaining TODO", () => {
        const todo = leaves(m, locale).filter(([, v]) => /\bTODO\b/.test(v));
        expect(todo).toEqual([]);
      });

      it("covers every group, unit and reprocess level", () => {
        for (const g of groupRegistry(paramRegistry.params)) expect(m.groups[g.group], `group ${g.group}`).toBeTruthy();
        for (const u of PARAM_UNITS) expect(m.units[u], `unit ${u}`).toBeDefined();
        for (const l of REPROCESS_LEVELS) expect(m.reprocess[l]).toBeTruthy();
      });

      it("keeps the English diagnostic disclaimer (PRV-6)", () => {
        expect(m.disclaimer).toContain("This is not a diagnostic tool.");
      });

      it("uses research wording only (ADR-0004)", () => {
        const banned = /diagnos(is|e|ed)\b(?! tool)|risk of autism|traffic/i;
        const hits = leaves(m, locale).filter(([, v]) => banned.test(v.replace("This is not a diagnostic tool.", "")));
        expect(hits).toEqual([]);
      });
    });
  }

  it("en and ne have the same shape", () => {
    expect(shape(researchMessages.ne)).toEqual(shape(researchMessages.en));
  });

  it("the bundled registry is the schema file of the repository", () => {
    const file = resolve(process.cwd(), "../../docs/spec/schemas/asd/param-registry-v1.json");
    if (!existsSync(file)) return;
    expect(JSON.parse(readFileSync(file, "utf8"))).toEqual(JSON.parse(readFileSync(resolve(process.cwd(), "src/lib/research/param-registry-v1.json"), "utf8")));
  });
});
