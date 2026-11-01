/**
 * Drift guard for the participant workspace schemas (WP-C.14), next to `contract.test.ts`: the zod schemas in
 * `participant.ts` must declare every property of docs/spec/schemas/asd/api-v1.json, require the required ones,
 * accept every published example and agree with its enums; the mock's responses must parse.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import type { ZodType } from "@/lib/zod";
import { participantSchema } from "./admin";
import { createMockClient, MOCK_ORG_ID } from "./mock";
import { sessionRawSchema } from "./types";
import {
  NOTE_KINDS,
  careSummarySchema,
  TIMELINE_KINDS,
  clinicalNotePageSchema,
  clinicalNoteSchema,
  createClinicalNoteRequestSchema,
  disclosureDeliverySchema,
  disclosurePageSchema,
  disclosureSchema,
  patientRefSchema,
  patientRefSearchRequestSchema,
  patientRefSearchResponseSchema,
  putPatientRefRequestSchema,
  timelineEntrySchema,
  timelinePageSchema,
} from "./participant";

interface Def {
  enum?: string[];
  examples?: unknown[];
  required?: string[];
  properties?: Record<string, unknown>;
}
const api = JSON.parse(
  readFileSync(resolve(process.cwd(), "../../docs/spec/schemas/asd/api-v1.json"), "utf8"),
) as { $defs: Record<string, Def> };
const defs = api.$defs;

type Shaped = ZodType & { shape: Record<string, ZodType & { isOptional(): boolean }> };

const SCHEMAS: Record<string, Shaped> = {
  Participant: participantSchema,
  CareSummary: careSummarySchema,
  ClinicalNote: clinicalNoteSchema,
  ClinicalNotePage: clinicalNotePageSchema,
  CreateClinicalNoteRequest: createClinicalNoteRequestSchema,
  TimelineEntry: timelineEntrySchema,
  TimelinePage: timelinePageSchema,
  PatientRef: patientRefSchema,
  PutPatientRefRequest: putPatientRefRequestSchema,
  PatientRefSearchRequest: patientRefSearchRequestSchema,
  PatientRefSearchResponse: patientRefSearchResponseSchema,
  Disclosure: disclosureSchema,
  DisclosurePage: disclosurePageSchema,
  DisclosureDelivery: disclosureDeliverySchema,
} as never;

describe("api-v1.json contract: participant workspace (WP-C.14)", () => {
  for (const [name, schema] of Object.entries(SCHEMAS)) {
    const def = defs[name];
    it(`${name}: zod declares every property and requires the required ones`, () => {
      expect(def, `${name} missing from api-v1.json`).toBeDefined();
      for (const p of Object.keys(def!.properties ?? {})) {
        expect(schema.shape, `${name}.${p} not declared in zod`).toHaveProperty(p);
      }
      for (const p of def!.required ?? []) {
        expect(schema.shape[p]!.isOptional(), `${name}.${p} is required by the contract`).toBe(false);
      }
      for (const example of def!.examples ?? []) expect(schema.parse(example)).toBeTruthy();
    });
  }

  it("enums match the contract", () => {
    expect([...NOTE_KINDS]).toEqual(defs.ClinicalNoteKind!.enum);
    expect([...TIMELINE_KINDS]).toEqual(defs.TimelineEntryKind!.enum);
  });

  it("the mock returns shapes the schemas accept", async () => {
    const mock = createMockClient();
    const first = (await mock.listParticipants(MOCK_ORG_ID, { limit: 100 })).items[0]!;
    participantSchema.parse(await mock.getParticipant(first.id));
    const timeline = timelinePageSchema.parse(await mock.getParticipantTimeline(first.id));
    expect(timeline.items.some((e) => e.kind === "session")).toBe(true);
    expect(timeline.items.some((e) => e.kind === "note")).toBe(true);
    careSummarySchema.parse(await mock.getParticipantCareSummary(first.id));
    const mine = (await mock.listSessions(MOCK_ORG_ID, { participant_id: first.id, limit: 1 })).items[0]!;
    sessionRawSchema.parse(await mock.getSessionRaw(mine.id));
    const notes = clinicalNotePageSchema.parse(await mock.listParticipantNotes(first.id));
    expect(notes.items.some((n) => n.corrected_by)).toBe(true);
    expect(disclosurePageSchema.parse(await mock.listParticipantDisclosures(first.id)).items).toHaveLength(1);
    clinicalNoteSchema.parse(await mock.createParticipantNote(first.id, { body: "Contract check" }));
    const ref = await mock.getPatientRef(first.id);
    if (ref) patientRefSchema.parse(ref);
    patientRefSearchResponseSchema.parse(
      await mock.searchPatientRef(MOCK_ORG_ID, { assigning_authority: "TUTH", mrn: "nope" }),
    );
  });
});
