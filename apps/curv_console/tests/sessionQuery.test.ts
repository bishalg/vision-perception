import { describe, expect, it } from "vitest";
import { parseSessionQuery, serializeSessionQuery } from "@/lib/sessionQuery";

describe("session query", () => {
  it("drops invalid values and keeps valid ones", () => {
    const q = parseSessionQuery({ flag: "purple", from: "yesterday", to: "2026-11-01", reviewed: "false", cursor: "20" });
    expect(q).toEqual({ to: "2026-11-01", reviewed: false, cursor: "20" });
  });
  it("round-trips", () => {
    const q = { flag: "red" as const, cursor: "5", reviewed: true };
    expect(parseSessionQuery(Object.fromEntries(new URLSearchParams(serializeSessionQuery(q))))).toEqual(q);
  });
  it("serialises empty as empty string", () => {
    expect(serializeSessionQuery({})).toBe("");
  });
});
