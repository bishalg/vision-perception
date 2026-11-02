/** WP-C.14: participant workspace. Wire format, mock rules, components and RBAC. */
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Disclosures } from "@/components/participant/Disclosures";
import { MrnPanel, type MrnPanelProps } from "@/components/participant/MrnPanel";
import { NotesPanel } from "@/components/participant/NotesPanel";
import { PrintSummary } from "@/components/participant/PrintSummary";
import { ParticipantTabs, parseWorkspaceTab } from "@/components/participant/ParticipantTabs";
import { Timeline, describeEntry, parseDay, parseKinds } from "@/components/participant/Timeline";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import { ROLES, type ClinicalReview } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import type { ClinicalNote, TimelineEntry } from "@/lib/api/participant";
import { can } from "@/lib/permissions";
import { participantErrorMessage } from "@/messages/participant";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
const problem = (status: number, slug: string) =>
  json({ type: `https://schemas.curv.org/asd/problems/${slug}`, title: slug, status }, status);

function client(response: Response) {
  const f = vi.fn().mockResolvedValue(response);
  return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch }) };
}

const note = (over: Partial<ClinicalNote> = {}): ClinicalNote => ({
  id: "n1", participant_id: "p1", session_id: null, kind: "general", body: "Original text", corrects_id: null,
  corrected_by: null, author_id: "u1", author_display_name: "Dr A", created_at: "2026-10-30T09:00:00.000Z", ...over,
});
const entry = (over: Partial<TimelineEntry> = {}): TimelineEntry => ({
  kind: "session", id: "s1", at: "2026-10-30T09:00:00.000Z", session_id: "s1", actor_id: null, actor_display_name: null,
  summary: { attention_flag: "red" }, ...over,
});

describe("RBAC", () => {
  it("notes are written by clinicians only; the MRN by owner, admin and clinician; none by the superadmin", () => {
    expect(ROLES.filter((r) => can(r, "note.write"))).toEqual(["clinician"]);
    expect(ROLES.filter((r) => can(r, "patientref.read")).sort()).toEqual(["admin", "clinician", "owner"]);
    expect(ROLES.filter((r) => can(r, "patientref.write")).sort()).toEqual(["admin", "clinician", "owner"]);
    expect(can("superadmin", "patientref.read")).toBe(false);
    expect(can("superadmin", "note.write")).toBe(false);
    expect(ROLES.filter((r) => can(r, "clinical.content")).sort()).toEqual(["clinician", "owner"]);
  });
});

describe("live client wire format", () => {
  it("reads detail and a timeline page with cursor and limit", async () => {
    const d = client(json({ id: "p/1", org_id: "o", participant_token: "child-001", session_count: 1 }));
    await d.api.getParticipant("p/1");
    expect(d.f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p%2F1");
    const t = client(json({ items: [entry()], next_cursor: null }));
    await t.api.getParticipantTimeline("p1", { cursor: "c", limit: 25 });
    expect(t.f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p1/timeline?cursor=c&limit=25");
  });

  it("sends timeline filters as query parameters, kinds as a comma list", async () => {
    const t = client(json({ items: [], next_cursor: null }));
    await t.api.getParticipantTimeline("p1", { kind: ["session", "note"], from: "2026-10-01", to: "2026-10-31", limit: 25 });
    expect(t.f.mock.calls[0]![0]).toBe(
      "https://api.test/v1/participants/p1/timeline?kind=session%2Cnote&from=2026-10-01&to=2026-10-31&limit=25",
    );
    const e = client(json({ items: [], next_cursor: null }));
    await e.api.getParticipantTimeline("p1", { kind: [] });
    expect(e.f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p1/timeline");
  });

  it("reads the care summary and the sessions of one participant", async () => {
    const s = client(json({ participant_token: "child-001", total_events: 0, task_completions: 0, medication_doses: 0, diary_entries: 0, milestones: 0, session_count: 0, checkin_count: 0, completion_rate_percent: 0, recent_moods: [] }));
    await s.api.getParticipantCareSummary("p/1");
    expect(s.f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p%2F1/summary");
    const l = client(json({ items: [], next_cursor: null, total: 0 }));
    await l.api.listSessions("o", { participant_id: "p1", limit: 21 });
    expect(l.f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/sessions?participant_id=p1&limit=21");
  });

  it("creates a note and a correction with POST bodies", async () => {
    const { f, api } = client(json(note({ kind: "correction", corrects_id: "n0" }), 201));
    await api.createParticipantNote("p1", { body: "x", corrects_id: "n0" });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p1/notes");
    expect(f.mock.calls[0]![1].method).toBe("POST");
    expect(JSON.parse(f.mock.calls[0]![1].body)).toEqual({ body: "x", corrects_id: "n0" });
  });

  it("MRN: PUT carries the number in the body only, search is a POST, 404 reads as null, 409 surfaces its slug", async () => {
    const put = client(json({ participant_id: "p1", assigning_authority: "TUTH", updated_at: "2026-10-30T09:00:00Z" }, 201));
    await put.api.putPatientRef("p1", { assigning_authority: "TUTH", mrn: "12-345" });
    expect(put.f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p1/patient-ref");
    expect(put.f.mock.calls[0]![0]).not.toContain("12-345");
    expect(put.f.mock.calls[0]![1].method).toBe("PUT");
    expect(JSON.parse(put.f.mock.calls[0]![1].body)).toEqual({ assigning_authority: "TUTH", mrn: "12-345" });

    const search = client(json({ participant_id: "p9" }));
    expect((await search.api.searchPatientRef("o", { assigning_authority: "TUTH", mrn: "12345" })).participant_id).toBe("p9");
    expect(search.f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/patient-refs/search");
    expect(search.f.mock.calls[0]![0]).not.toContain("12345");
    expect(search.f.mock.calls[0]![1].method).toBe("POST");

    expect(await client(problem(404, "not-found")).api.getPatientRef("p1")).toBeNull();
    await expect(client(problem(409, "patient-ref-disabled")).api.getPatientRef("p1")).rejects.toMatchObject({
      status: 409,
      slug: "patient-ref-disabled",
    });
    const del = client(new Response(null, { status: 204 }));
    await del.api.deletePatientRef("p1");
    expect(del.f.mock.calls[0]![1].method).toBe("DELETE");
  });

  it("lists disclosures", async () => {
    const { f, api } = client(json({ items: [], next_cursor: null }));
    await api.listParticipantDisclosures("p1", { limit: 10 });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/participants/p1/disclosures?limit=10");
  });
});

describe("mock client", () => {
  async function first() {
    const api = createMockClient();
    const p = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items[0]!;
    return { api, p };
  }

  it("pages the timeline newest first without repeats", async () => {
    const { api, p } = await first();
    const seen: string[] = [];
    let cursor: string | undefined;
    let last = "9999";
    do {
      const page = await api.getParticipantTimeline(p.id, { cursor, limit: 2 });
      for (const e of page.items) {
        expect(e.at <= last).toBe(true);
        last = e.at;
        seen.push(`${e.kind}:${e.id}`);
      }
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
    expect(new Set(seen).size).toBe(seen.length);
    expect(seen.length).toBeGreaterThan(3);
  });

  it("filters the timeline by kind and range, and the cursor continues inside the filter", async () => {
    const { api, p } = await first();
    const all = (await api.getParticipantTimeline(p.id, { limit: 100 })).items;
    const sessions = (await api.getParticipantTimeline(p.id, { kind: ["session"], limit: 100 })).items;
    expect(sessions.length).toBeGreaterThan(0);
    expect(sessions.every((e) => e.kind === "session")).toBe(true);
    expect(sessions).toEqual(all.filter((e) => e.kind === "session"));
    const two = (await api.getParticipantTimeline(p.id, { kind: ["session", "note"], limit: 100 })).items;
    expect(two).toEqual(all.filter((e) => e.kind === "session" || e.kind === "note"));
    const seen: string[] = [];
    let cursor: string | undefined;
    do {
      const page = await api.getParticipantTimeline(p.id, { kind: ["session"], limit: 1, cursor });
      seen.push(...page.items.map((e) => e.id));
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
    expect(seen).toEqual(sessions.map((e) => e.id));
    // A bare `to` day is inclusive.
    const day = all[0]!.at.slice(0, 10);
    const onDay = (await api.getParticipantTimeline(p.id, { from: day, to: day, limit: 100 })).items;
    expect(onDay.length).toBeGreaterThan(0);
    expect(onDay.every((e) => e.at.startsWith(day))).toBe(true);
    await expect(api.getParticipantTimeline(p.id, { from: "yesterday" })).rejects.toMatchObject({ status: 400 });
  });

  it("lists the sessions of one participant and reads its care summary and a session's raw record", async () => {
    const { api, p } = await first();
    const mine = await api.listSessions(MOCK_ORG_ID, { participant_id: p.id, limit: 100 });
    expect(mine.items.length).toBeGreaterThan(0);
    expect(mine.items.every((s) => s.participant_id === p.id)).toBe(true);
    expect(mine.total).toBe(mine.items.length);
    expect((await api.getParticipantCareSummary(p.id)).participant_token).toBe(p.participant_token);
    const raw = await api.getSessionRaw(mine.items[0]!.id);
    expect(raw.participant_token).toBe(p.participant_token);
    expect(raw.raw_payload).not.toHaveProperty("notes");
    await expect(api.getSessionRaw("nope")).rejects.toMatchObject({ status: 404 });
  });

  it("notes are append-only: a correction links both ways and the original reports corrected_by", async () => {
    const { api, p } = await first();
    const before = (await api.listParticipantNotes(p.id)).items;
    const target = before.find((n) => !n.corrected_by)!;
    const fix = await api.createParticipantNote(p.id, { body: "Fixed", corrects_id: target.id });
    expect([fix.kind, fix.corrects_id]).toEqual(["correction", target.id]);
    const after = (await api.listParticipantNotes(p.id)).items;
    expect(after).toHaveLength(before.length + 1);
    expect(after.find((n) => n.id === target.id)!.corrected_by).toBe(fix.id);
    expect(after.find((n) => n.id === target.id)!.body).toBe(target.body);
  });

  it("validates notes: empty body, foreign session and unknown correction target are 400", async () => {
    const { api, p } = await first();
    await expect(api.createParticipantNote(p.id, { body: "  " })).rejects.toMatchObject({ status: 400 });
    await expect(api.createParticipantNote(p.id, { body: "x", session_id: "nope" })).rejects.toMatchObject({ status: 400 });
    await expect(api.createParticipantNote(p.id, { body: "x", corrects_id: "nope" })).rejects.toMatchObject({ status: 400 });
    await expect(api.getParticipant("nope")).rejects.toMatchObject({ status: 404 });
  });

  it("MRN: link, case-insensitive duplicate is 409, search normalises, unlink removes it", async () => {
    const api = createMockClient();
    const [a, b] = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.filter((x) => !x.has_patient_ref);
    await api.putPatientRef(a!.id, { assigning_authority: " tuth ", mrn: "12-345" });
    expect((await api.getPatientRef(a!.id))!.assigning_authority).toBe("TUTH");
    expect((await api.getParticipant(a!.id)).has_patient_ref).toBe(true);
    await expect(api.putPatientRef(b!.id, { assigning_authority: "TUTH", mrn: "12 345" })).rejects.toMatchObject({ status: 409 });
    expect(await api.searchPatientRef(MOCK_ORG_ID, { assigning_authority: "TUTH", mrn: "12345" })).toEqual({ participant_id: a!.id });
    expect(await api.searchPatientRef(MOCK_ORG_ID, { assigning_authority: "TUTH", mrn: "99" })).toEqual({ participant_id: null });
    await api.deletePatientRef(a!.id);
    expect(await api.getPatientRef(a!.id)).toBeNull();
    await expect(api.deletePatientRef(a!.id)).rejects.toMatchObject({ status: 404 });
    await expect(api.putPatientRef(a!.id, { assigning_authority: "TUTH", mrn: "   " })).rejects.toMatchObject({ status: 400 });
  });
});

describe("Timeline", () => {
  const entries = [entry(), entry({ kind: "note", id: "n1", summary: { note_kind: "general" }, actor_display_name: "Dr A" })];

  it("shows what the server filtered and links back with the filter kept", () => {
    expect(parseKinds(["note", "bogus", "note"])).toEqual(["note"]);
    expect(parseDay("2026-10-05")).toBe("2026-10-05");
    expect(parseDay(["2026-10-05", "x"])).toBe("2026-10-05");
    expect(parseDay("yesterday")).toBeUndefined();
    const served = [entries[1]!];
    render(
      <Timeline entries={served} kinds={["note"]} from="2026-10-01" to="2026-10-31" basePath="/o/participants/p1" orgPath="/o" nextCursor="abc" hasCursor />,
    );
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveAttribute("data-kind", "note");
    expect(screen.getByRole("link", { name: "Older" })).toHaveAttribute(
      "href",
      "/o/participants/p1?tab=timeline&kind=note&from=2026-10-01&to=2026-10-31&cursor=abc",
    );
    expect(screen.getByRole("link", { name: "Newest" })).toHaveAttribute(
      "href",
      "/o/participants/p1?tab=timeline&kind=note&from=2026-10-01&to=2026-10-31",
    );
    expect(screen.getByRole("checkbox", { name: "Note" })).toBeChecked();
    expect(screen.getByLabelText("From")).toHaveValue("2026-10-01");
    // The page's rows are not filtered again on the client, and the old page-only caveat is gone.
    expect(screen.queryByText(/apply to the entries on this page/)).toBeNull();
    cleanup();
    render(<Timeline entries={entries} kinds={["note"]} basePath="/o/participants/p1" orgPath="/o" nextCursor={null} hasCursor={false} />);
    expect(screen.getAllByRole("row")).toHaveLength(3);
  });

  it("shows all entries without a filter, links sessions, and describes headers only", () => {
    render(<Timeline entries={entries} kinds={[]} basePath="/o/participants/p1" orgPath="/o" nextCursor={null} hasCursor={false} />);
    expect(screen.getAllByRole("row")).toHaveLength(3);
    expect(screen.getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/o/sessions/s1");
    expect(screen.queryByRole("link", { name: "Older" })).toBeNull();
    expect(describeEntry(entry({ kind: "clinical_review", summary: { state: "awaiting_cosign", version_no: 2 } }))).toBe("Review awaiting cosign, version 2");
    expect(describeEntry(entry({ kind: "care_event", summary: { care_kind: "diary" } }))).toBe("Care event: diary");
  });

  it("explains an empty filtered page differently from an empty timeline", () => {
    render(<Timeline entries={[]} kinds={["letter"]} basePath="/o/participants/p1" orgPath="/o" nextCursor={null} hasCursor={false} />);
    expect(screen.getByText("No entries match the chosen filters.")).toBeInTheDocument();
  });

  it("explains an empty page", () => {
    render(<Timeline entries={[]} kinds={[]} basePath="/o/participants/p1" orgPath="/o" nextCursor={null} hasCursor={false} />);
    expect(screen.getByText("Nothing on the timeline yet.")).toBeInTheDocument();
  });
});

describe("NotesPanel", () => {
  it("shows the corrected original struck through and links to its correction", () => {
    const original = note({ corrected_by: "n2" });
    const fix = note({ id: "n2", kind: "correction", corrects_id: "n1", body: "Fixed text" });
    render(<NotesPanel notes={[fix, original]} sessions={[]} canWrite create={vi.fn()} />);
    const item = document.getElementById("note-n1")!;
    expect(item).toHaveAttribute("data-corrected", "true");
    expect(item.querySelector("s")).toHaveTextContent("Original text");
    expect(within(item).queryByRole("button", { name: /Correct/ })).toBeNull();
    expect(within(item).getByRole("link", { name: "Corrected by a later note" })).toHaveAttribute("href", "#note-n2");
    expect(document.getElementById("note-n2")!.querySelector("s")).toBeNull();
  });

  it("adds a note through the form and sends the text to the action only", async () => {
    const create = vi.fn().mockResolvedValue({ ok: true });
    render(<NotesPanel notes={[]} sessions={[{ id: "s1", label: "2026-10-30 09:00 UTC" }]} canWrite create={create} />);
    await userEvent.type(screen.getByLabelText("Note"), "Seen today");
    await userEvent.selectOptions(screen.getByLabelText("About session"), "s1");
    await userEvent.click(screen.getByRole("button", { name: "Add note" }));
    await waitFor(() => expect(create).toHaveBeenCalled());
    const form = create.mock.calls[0]![0] as FormData;
    expect([form.get("body"), form.get("session_id")]).toEqual(["Seen today", "s1"]);
    expect(await screen.findByText("Note saved.")).toBeInTheDocument();
  });

  it("corrects a note: the form posts corrects_id and Cancel closes it", async () => {
    const create = vi.fn().mockResolvedValue({ ok: true });
    render(<NotesPanel notes={[note()]} sessions={[]} canWrite create={create} />);
    await userEvent.click(screen.getByRole("button", { name: /^Correct:/ }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("button", { name: "Save correction" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: /^Correct:/ }));
    const area = screen.getByRole("textbox", { name: "Correct this note" });
    await userEvent.clear(area);
    await userEvent.type(area, "Better text");
    await userEvent.click(screen.getByRole("button", { name: "Save correction" }));
    await waitFor(() => expect(create).toHaveBeenCalled());
    const form = create.mock.calls[0]![0] as FormData;
    expect([form.get("corrects_id"), form.get("body")]).toEqual(["n1", "Better text"]);
  });

  it("read-only for non-clinicians: no form, no Correct button; failures are shown", async () => {
    const { rerender } = render(<NotesPanel notes={[note()]} sessions={[]} canWrite={false} create={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Add note" })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Correct:/ })).toBeNull();
    expect(screen.getByText("Only clinicians can add notes.")).toBeInTheDocument();
    const create = vi.fn().mockResolvedValue({ ok: false, slug: "validation-failed", status: 400, params: "body: must not be empty" });
    rerender(<NotesPanel notes={[]} sessions={[]} canWrite create={create} />);
    await userEvent.type(screen.getByLabelText("Note"), "x");
    await userEvent.click(screen.getByRole("button", { name: "Add note" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("body: must not be empty");
  });
});

describe("MrnPanel", () => {
  const base = (over: Partial<MrnPanelProps> = {}): MrnPanelProps => ({
    mode: "mrn",
    hasRef: true,
    canRead: true,
    canWrite: true,
    reveal: vi.fn().mockResolvedValue({ ok: true, authority: "TUTH", mrn: "12-345", updatedAt: "2026-10-30T09:00:00Z" }),
    save: vi.fn().mockResolvedValue({ ok: true }),
    unlink: vi.fn().mockResolvedValue({ ok: true }),
    search: vi.fn().mockResolvedValue({ ok: true, found: false }),
    ...over,
  });

  it("says disabled when the org does not store MRNs and offers no form", () => {
    render(<MrnPanel {...base({ mode: "none" })} />);
    expect(screen.getByTestId("mrn-disabled")).toHaveTextContent("does not store hospital patient references");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("does not decrypt until Show MRN is pressed, then can hide it again", async () => {
    const p = base();
    render(<MrnPanel {...p} />);
    expect(screen.queryByTestId("mrn-value")).toBeNull();
    expect(p.reveal).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Show MRN" }));
    expect(await screen.findByTestId("mrn-value")).toHaveTextContent("12-345");
    await userEvent.click(screen.getByRole("button", { name: "Hide MRN" }));
    expect(screen.queryByTestId("mrn-value")).toBeNull();
  });

  it("links an MRN with a form POST to the action", async () => {
    const p = base({ hasRef: false });
    render(<MrnPanel {...p} />);
    expect(screen.getByTestId("mrn-none")).toBeInTheDocument();
    const form = screen.getByRole("form", { name: "Link an MRN" });
    await userEvent.type(within(form).getByLabelText("Assigning authority"), "TUTH");
    await userEvent.type(within(form).getByLabelText("MRN"), "12-345");
    await userEvent.click(within(form).getByRole("button", { name: "Save MRN" }));
    await waitFor(() => expect(p.save).toHaveBeenCalled());
    const data = (p.save as ReturnType<typeof vi.fn>).mock.calls[0]![0] as FormData;
    expect([data.get("assigning_authority"), data.get("mrn")]).toEqual(["TUTH", "12-345"]);
    expect(await screen.findByText("MRN saved.")).toBeInTheDocument();
  });

  it("unlinks only after an in-page confirmation", async () => {
    const p = base();
    render(<MrnPanel {...p} />);
    await userEvent.click(screen.getByRole("button", { name: "Unlink MRN" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("Remove the MRN link");
    expect(p.unlink).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Yes, unlink" }));
    await waitFor(() => expect(p.unlink).toHaveBeenCalled());
  });

  it("search is a form action; a miss says so and a failure shows the message", async () => {
    const p = base({ search: vi.fn().mockResolvedValueOnce({ ok: true, found: false }).mockResolvedValueOnce({ ok: false, slug: "patient-ref-disabled", status: 409 }) });
    render(<MrnPanel {...p} />);
    const form = screen.getByRole("form", { name: "Find a participant by MRN" });
    await userEvent.type(within(form).getByLabelText("Assigning authority"), "TUTH");
    await userEvent.type(within(form).getByLabelText("MRN"), "999");
    await userEvent.click(within(form).getByRole("button", { name: "Search" }));
    expect(await screen.findByText("No participant has that MRN.")).toBeInTheDocument();
    await userEvent.type(within(form).getByLabelText("Assigning authority"), "TUTH");
    await userEvent.type(within(form).getByLabelText("MRN"), "999");
    await userEvent.click(within(form).getByRole("button", { name: "Search" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("does not store hospital patient references");
  });

  it("a read-only role sees no write controls; a reveal failure is shown", async () => {
    const p = base({ canWrite: false, reveal: vi.fn().mockResolvedValue({ ok: false, slug: "forbidden", status: 403 }) });
    render(<MrnPanel {...p} />);
    expect(screen.queryByRole("button", { name: "Unlink MRN" })).toBeNull();
    expect(screen.queryByRole("form", { name: "Replace the MRN" })).toBeNull();
    expect(screen.queryByRole("form", { name: "Link an MRN" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Show MRN" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Your role cannot do this.");
  });

  it("explains a role without MRN access", () => {
    render(<MrnPanel {...base({ canRead: false })} />);
    expect(screen.getByText(/cannot view or change patient references/)).toBeInTheDocument();
  });
});

describe("Disclosures", () => {
  it("lists exports with deliveries, or says nothing was exported", () => {
    const { rerender } = render(<Disclosures items={[]} basePath="/o/participants/p1" nextCursor={null} hasCursor={false} />);
    expect(screen.getByText(/has not been exported/)).toBeInTheDocument();
    rerender(
      <Disclosures
        basePath="/o/participants/p1"
        nextCursor="c1"
        hasCursor={false}
        items={[{
          export_id: "e1", kind: "bulk", format: "csv_excel", mode: "identified", purpose: "clinical_care", status: "succeeded",
          requested_by: "Dr A", created_at: "2026-10-30T09:00:00Z", session_ids: ["s1", "s2"],
          deliveries: [{ destination_id: "d1", destination_name: "Hospital SFTP", status: "delivered", delivered_at: null }],
        }]}
      />,
    );
    expect(screen.getByText("Identified")).toBeInTheDocument();
    expect(screen.getByText("Clinical care")).toBeInTheDocument();
    expect(screen.getByText("Hospital SFTP: delivered")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Older" })).toHaveAttribute("href", "/o/participants/p1?tab=disclosures&cursor=c1");
  });
});

describe("PrintSummary", () => {
  const participant = { id: "p1", org_id: "o", participant_token: "child-001", age_bracket_at_enrol: "24-36m", session_count: 3, withdrawn_at: null };
  const review = (state: ClinicalReview["state"]): ClinicalReview =>
    ({
      id: "r1", org_id: "o", session_id: "s1", state, cosign_required: false, row_version: 1, content_hash: "h",
      current: { version_no: 1, status: "signed", created_at: "2026-10-30T09:00:00Z", disposition: "monitor", impression: "Signed impression" },
      signatures: [{ id: "g", version_no: 1, role: "sign", signer_id: "u", signer_display_name: "Dr Sign", meaning: "m", content_hash: "h", signed_at: "2026-10-30T10:00:00Z" }],
      created_at: "2026-10-30T09:00:00Z", updated_at: "2026-10-30T09:00:00Z",
    }) as ClinicalReview;

  it("prints signed content, labels drafts and never prints an unsigned impression", () => {
    const draft = review("in_progress");
    draft.current.impression = "Draft impression";
    render(
      <PrintSummary
        participant={participant}
        generatedAt="2026-11-01T00:00:00Z"
        notes={[note()]}
        care={{ participant_token: "child-001", total_events: 5, task_completions: 2, medication_doses: 1, diary_entries: 1, milestones: 1, session_count: 2, checkin_count: 0, completion_rate_percent: 100, recent_moods: ["calm"] }}
        rows={[
          { sessionId: "s1", at: "2026-10-30T09:00:00Z", flag: "red", review: review("final") },
          { sessionId: "s2", at: "2026-10-29T09:00:00Z", flag: "green", review: draft },
          { sessionId: "s3", at: "2026-10-28T09:00:00Z", flag: null, review: null },
        ]}
      />,
    );
    expect(screen.getByText("Signed impression")).toBeInTheDocument();
    expect(screen.getByText("Signed by Dr Sign")).toBeInTheDocument();
    expect(screen.getByText("Monitor")).toBeInTheDocument();
    expect(screen.queryByText("Draft impression")).toBeNull();
    expect(screen.getByText("Draft (unsigned)")).toBeInTheDocument();
    expect(screen.getByText("Not reviewed")).toBeInTheDocument();
    expect(screen.getByText("Original text")).toBeInTheDocument();
    expect(screen.getByText(/5 care events recorded: 2 tasks completed.*Recent moods: calm/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Print" })).toBeInTheDocument();
  });

  it("without content access: no notes section and review cells say so", () => {
    render(
      <PrintSummary
        participant={participant}
        generatedAt="2026-11-01T00:00:00Z"
        notes={undefined}
        rows={[{ sessionId: "s1", at: "2026-10-30T09:00:00Z", flag: "red", review: undefined }]}
      />,
    );
    expect(screen.queryByText("Clinical notes")).toBeNull();
    expect(screen.getByText("Not shown for your role")).toBeInTheDocument();
  });
});

describe("tabs, table and messages", () => {
  it("parses the tab and falls back to the timeline for a hidden or unknown one", () => {
    expect(parseWorkspaceTab("notes")).toBe("notes");
    expect(parseWorkspaceTab("notes", ["timeline", "print"])).toBe("timeline");
    expect(parseWorkspaceTab(["print"])).toBe("print");
    expect(parseWorkspaceTab("zzz")).toBe("timeline");
  });

  it("WP-C.18: Visits and Files are workspace tabs", () => {
    expect(parseWorkspaceTab("visits")).toBe("visits");
    expect(parseWorkspaceTab("attachments", ["timeline", "visits"])).toBe("timeline");
    render(<ParticipantTabs basePath="/o/participants/p1" active="visits" tabs={["timeline", "visits", "attachments"]} />);
    expect(screen.getAllByRole("link").map((a) => a.textContent)).toEqual(["Timeline", "Visits", "Files"]);
    expect(screen.getByRole("link", { name: "Files" })).toHaveAttribute("href", "/o/participants/p1?tab=attachments");
  });

  it("WP-C.18: the timeline links visit and file entries to their tabs, never shows their free text", () => {
    render(
      <Timeline
        entries={[
          entry({ kind: "encounter", id: "e1", session_id: null, summary: { encounter_kind: "phone_call", status: "open", reason: "hidden words" } }),
          entry({ kind: "attachment", id: "a1", session_id: null, summary: { attachment_kind: "report", file_name: "r.pdf", byte_size: 4096 } }),
        ]}
        kinds={[]}
        basePath="/o/participants/p1"
        orgPath="/o"
        nextCursor={null}
        hasCursor={false}
      />,
    );
    expect(screen.getByText(/Phone call \(open\)/)).toBeInTheDocument();
    expect(screen.getByText(/Report, r\.pdf, 4 KB/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open visits" })).toHaveAttribute("href", "/o/participants/p1?tab=visits");
    expect(screen.getByRole("link", { name: "Open files" })).toHaveAttribute("href", "/o/participants/p1?tab=attachments");
    expect(document.body.textContent).not.toContain("hidden words");
  });

  it("renders only the permitted tabs and marks the active one", () => {
    render(<ParticipantTabs basePath="/o/participants/p1" active="print" tabs={["timeline", "print"]} />);
    expect(screen.getAllByRole("link").map((a) => a.textContent)).toEqual(["Timeline", "Print summary"]);
    expect(screen.getByRole("link", { name: "Print summary" })).toHaveAttribute("aria-current", "page");
  });

  it("the participants list links each token to its workspace", () => {
    render(
      <ParticipantsTable
        orgId="o 1"
        participants={[{ id: "p/1", org_id: "o", participant_token: "child-001", session_count: 1 }]}
      />,
    );
    expect(screen.getByRole("link", { name: "child-001" })).toHaveAttribute("href", "/o%201/participants/p%2F1");
  });

  it("maps error slugs to text", () => {
    expect(participantErrorMessage("patient-ref-disabled", 409)).toMatch(/does not store/);
    expect(participantErrorMessage(null, 409)).toMatch(/already linked/);
    expect(participantErrorMessage(null, 0)).toMatch(/Could not reach/);
  });
});
