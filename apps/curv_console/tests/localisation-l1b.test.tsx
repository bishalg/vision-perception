/**
 * WP-L.1b (ADR-0013): the participant, visit, file, follow-up, worklist and clinical-review components render the
 * Nepali bundle under `LocaleProvider locale="ne"` (domain labels from the generated catalogue, no raw enum values,
 * problem sentences from `problem.<slug>`, the ADR-0004 sentence in both languages) and stay English without one.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ReviewPanel } from "@/components/clinical/ReviewPanel";
import { SignDialog } from "@/components/clinical/SignDialog";
import { WorklistFilters, WorklistTable } from "@/components/clinical/WorklistTable";
import { Disclosures } from "@/components/participant/Disclosures";
import { EncountersPanel } from "@/components/participant/EncountersPanel";
import { FollowUpsTable } from "@/components/participant/FollowUpsTable";
import { Timeline, describeEntry } from "@/components/participant/Timeline";
import { ageBracketLabel, languageLabel } from "@/components/participant/labels";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import type { ReviewActions, ReviewResult } from "@/components/clinical/types";
import type { ClinicalReview, WorklistItem } from "@/lib/api/clinical";
import type { Encounter } from "@/lib/api/emr";
import { LocaleProvider } from "@/lib/i18n/provider";
import { messagesFor } from "@/lib/i18n/messages";
import { clinicalErrorMessage } from "@/messages/clinical";
import { dt } from "@/messages/domain";
import { participantErrorMessage } from "@/messages/participant";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), back: vi.fn(), refresh: vi.fn() }) }));

const ne = <T,>(node: T) => <LocaleProvider locale="ne">{node as React.ReactNode}</LocaleProvider>;
const DEVANAGARI = /[ऀ-ॿ]/;

const encounter = (over: Partial<Encounter> = {}): Encounter => ({
  id: "e1", participant_id: "p1", kind: "screening_visit", occurred_at: "2026-10-30T09:00:00.000Z", clinician_id: "u1",
  clinician_display_name: "Dr A", reason: "Repeat observation", summary: null, has_summary: false, status: "open",
  follow_up_due_at: null, follow_up_done_at: null, session_ids: [], row_version: 3, created_at: "2026-10-30T09:00:00.000Z",
  updated_at: null, ...over,
});
const ok = () => Promise.resolve({ ok: true as const });
const actions = { create: vi.fn(ok), update: vi.fn(ok), state: vi.fn(ok), session: vi.fn(ok) };

describe("participant workspace in ne", () => {
  it("shows encounter_kind.screening_visit in Nepali and no raw enum value", () => {
    const { container } = render(
      ne(<EncountersPanel encounters={[encounter()]} sessions={[]} canWrite canSummary canContent canReopen actions={actions} now={0} orgPath="/o" />),
    );
    const label = dt("ne", "encounter_kind.screening_visit");
    expect(label).toMatch(DEVANAGARI);
    expect(within(screen.getByTestId("visit")).getByText(label)).toBeInTheDocument();
    expect(within(screen.getByTestId("visit")).getByText(dt("ne", "encounter_status.open"))).toBeInTheDocument();
    expect(container.textContent).not.toContain("screening_visit");
    expect(screen.getByRole("heading", { name: messagesFor("ne").participant.visits.title })).toBeInTheDocument();
  });

  it("describes timeline entries with catalogue labels", () => {
    const t = messagesFor("ne");
    const base = { id: "x", at: "2026-10-30T09:00:00Z", actor_display_name: null };
    expect(describeEntry({ ...base, kind: "encounter", summary: { encounter_kind: "home_visit", status: "open" } }, t)).toBe(
      `${dt("ne", "encounter_kind.home_visit")} (${dt("ne", "encounter_status.open")})`,
    );
    expect(describeEntry({ ...base, kind: "attachment", summary: { attachment_kind: "referral", file_name: "r.pdf", byte_size: 2048 } }, t)).toBe(
      `${dt("ne", "attachment_kind.referral")}, r.pdf, 2 KB`,
    );
    expect(describeEntry({ ...base, kind: "care_event", summary: { care_kind: "diary" } }, t)).toMatch(DEVANAGARI);
  });

  it("renders the Nepali timeline filter labels from the catalogue", () => {
    render(ne(<Timeline entries={[]} kinds={[]} basePath="/o/participants/p1" orgPath="/o" nextCursor={null} hasCursor={false} />));
    expect(screen.getByLabelText(dt("ne", "timeline_entry_kind.encounter"))).toBeInTheDocument();
    expect(screen.queryByLabelText("Visit")).toBeNull();
  });

  it("formats the age group from its months and the language with Intl.DisplayNames (Latin digits)", () => {
    expect(ageBracketLabel("ne", "24–35 mo")).toBe("24–35 महिना");
    expect(ageBracketLabel("ne", "48+ mo")).toBe("48+ महिना");
    expect(ageBracketLabel("ne", "unknown")).toBe("unknown");
    expect(ageBracketLabel("en", "24–35 mo")).toBe("24–35 mo");
    expect(languageLabel("ne", "ne")).toMatch(DEVANAGARI);
    expect(languageLabel("en", "ne")).toBe("Nepali");
    const row = {
      id: "p1", org_id: "o", participant_token: "child-001", age_bracket_at_enrol: "24–35 mo", language: "ne", session_count: 2,
      last_session_at: null, withdrawn_at: null,
    };
    render(ne(<ParticipantsTable participants={[row]} orgId="o" />));
    const cells = within(screen.getAllByRole("row")[1]!);
    expect(cells.getByText("24–35 महिना")).toBeInTheDocument();
    expect(cells.queryByText("ne")).toBeNull();
  });

  it("lists follow-ups with the visit kind label", () => {
    render(
      ne(
        <FollowUpsTable
          rows={[{ encounter_id: "e1", participant_id: "p1", participant_token: "child-001", kind: "phone_call", occurred_at: "2026-10-30T09:00:00Z", follow_up_due_at: "2026-11-02T00:00:00Z", reason: null, clinician_display_name: null }]}
          now={0}
          orgPath="/o"
          canWrite
          done={vi.fn(ok)}
          filtered={false}
        />,
      ),
    );
    expect(screen.getByText(new RegExp(dt("ne", "encounter_kind.phone_call")))).toBeInTheDocument();
  });

  it("shows disclosures with Nepali mode and purpose labels", () => {
    render(
      ne(
        <Disclosures
          basePath="/o/participants/p1"
          nextCursor={null}
          hasCursor={false}
          items={[{
            export_id: "e1", kind: "bulk", format: "csv_excel", mode: "identified", purpose: "clinical_care", status: "succeeded",
            requested_by: "Dr A", created_at: "2026-10-30T09:00:00Z", session_ids: [],
            deliveries: [],
          }]}
        />,
      ),
    );
    expect(screen.getByText(dt("ne", "export_mode.identified"))).toBeInTheDocument();
    expect(screen.getByText(dt("ne", "export_purpose.clinical_care"))).toBeInTheDocument();
    expect(screen.queryByText("succeeded")).toBeNull();
  });

  it("answers failures in Nepali by locale and in English by default", () => {
    expect(participantErrorMessage("attachment-too-large", 400)).toMatch(/10 MB/);
    expect(participantErrorMessage("attachment-too-large", 400, "ne")).toMatch(DEVANAGARI);
    expect(participantErrorMessage(null, 0, "ne")).not.toBe(participantErrorMessage(null, 0));
  });
});

const item = (over: Partial<WorklistItem> = {}): WorklistItem => ({
  id: "SESS-1", participant_id: "p1", participant_token: "child-001", started_at: "2026-11-01T08:00:00Z",
  duration_sec: 200, quality_verdict: "optimal", attention_flag: "red", age_hours: 30, assigned_to: null,
  clinical_state: null, cosign_required: true, has_patient_ref: true, review_id: null, ...over,
});

describe("worklist in ne", () => {
  it("renders states, flags and quality verdicts in Nepali", () => {
    const t = messagesFor("ne");
    const { container } = render(ne(<WorklistTable orgId="o" items={[item()]} />));
    const row = within(screen.getAllByRole("row")[1]!);
    expect(row.getByText(t.clinical.worklist.states.none)).toBeInTheDocument();
    // The flag badge itself is WP-L.1a's component.
    expect(container.querySelector(".badge-red")).not.toBeNull();
    expect(row.getByText(dt("ne", "quality_verdict.optimal"))).toBeInTheDocument();
    expect(container.textContent).not.toMatch(/optimal/);
    expect(row.getByText(t.clinical.worklist.hours(30))).toBeInTheDocument();
  });

  it("offers the flag filter with catalogue labels", () => {
    render(ne(<WorklistFilters />));
    expect(screen.getByRole("option", { name: dt("ne", "attention_flag.yellow") })).toBeInTheDocument();
  });
});

const HASH = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
function review(): ClinicalReview {
  return {
    id: "rev-1", org_id: "o", session_id: "s", state: "in_progress", assigned_to: "u", cosign_required: false,
    row_version: 4, content_hash: HASH, signatures: [], created_at: "2026-11-01T00:00:00Z", updated_at: "2026-11-01T00:00:00Z",
    current: { version_no: 1, status: "draft", created_at: "2026-11-01T00:00:00Z", disposition: "monitor", impression: "x", recommendations: "y" },
  };
}
const reviewActions = (): ReviewActions => ({
  claim: vi.fn(), save: vi.fn(), sign: vi.fn(), cosign: vi.fn(), amend: vi.fn(), reload: vi.fn(),
});
const NOT_DIAGNOSTIC = "This is not a diagnostic tool.";

describe("clinical review in ne", () => {
  it("shows the ADR-0004 sentence in Nepali and in English (I18N-23), and once in English", () => {
    const props = { review: review(), canWrite: true, canSign: true, canCosign: false, canContent: true, actions: reviewActions() };
    const { unmount } = render(ne(<ReviewPanel {...props} />));
    const box = screen.getByTestId("review-disclaimer");
    expect(box).toHaveTextContent(messagesFor("ne").clinical.review.disclaimer);
    expect(box).toHaveTextContent(NOT_DIAGNOSTIC);
    expect(box.textContent).toMatch(DEVANAGARI);
    unmount();
    render(<ReviewPanel {...props} />);
    expect(screen.getByTestId("review-disclaimer").textContent).toBe(NOT_DIAGNOSTIC);
  });

  it("shows disposition and state labels in Nepali", () => {
    const props = { review: review(), canWrite: true, canSign: true, canCosign: false, canContent: true, actions: reviewActions() };
    render(ne(<ReviewPanel {...props} />));
    expect(screen.getByRole("option", { name: dt("ne", "disposition.refer_for_evaluation") })).toBeInTheDocument();
    expect(screen.getByTestId("review-state")).toHaveTextContent(messagesFor("ne").clinical.review.states.in_progress);
  });

  it.each(["content-changed", "stale-version", "reauthentication-failed", "cosigner-must-differ", "signer-profile-incomplete"] as const)(
    "the sign dialog renders problem.%s in ne",
    async (slug) => {
      const user = userEvent.setup();
      const submit = vi.fn().mockResolvedValue({ ok: false, slug, status: 409 } satisfies ReviewResult);
      render(ne(<SignDialog review={review()} submit={submit} onDone={vi.fn()} onClose={vi.fn()} onReload={vi.fn()} />));
      await user.type(screen.getByLabelText(messagesFor("ne").clinical.dialog.password), "pw");
      await user.click(screen.getByLabelText(messagesFor("ne").clinical.dialog.ack));
      await user.click(screen.getByRole("button", { name: messagesFor("ne").clinical.dialog.confirmSign }));
      expect(await screen.findByRole("alert")).toHaveTextContent(dt("ne", `problem.${slug}`));
      expect(clinicalErrorMessage(slug, 409, "ne")).toBe(dt("ne", `problem.${slug}`));
    },
  );

  it("shows the Nepali statement and keeps the English sentence the server stores", () => {
    render(ne(<SignDialog review={review()} submit={vi.fn()} onDone={vi.fn()} onClose={vi.fn()} onReload={vi.fn()} />));
    expect(screen.getByText(messagesFor("ne").clinical.dialog.meaning)).toBeInTheDocument();
    expect(screen.getByText("I have reviewed this observation record and approve this review.")).toBeInTheDocument();
  });

  it("falls back to the console's own text for a slug the catalogue lacks", () => {
    expect(clinicalErrorMessage("no-such-slug", 0, "ne")).toBe(messagesFor("ne").clinical.errors.unavailable);
    expect(clinicalErrorMessage("no-such-slug", 500)).toBe(messagesFor("en").clinical.errors.fallback);
  });
});
