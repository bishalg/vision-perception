/**
 * WP-L.1a (I18N-15..19): the shell, sessions, review, devices and audit components render the `ne` tree and the domain
 * catalogue under `LocaleProvider locale="ne"`, with Latin digits, and keep English without a provider.
 */
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AuditTable } from "@/components/AuditTable";
import { DevicesTable } from "@/components/DevicesTable";
import { FlagBadge } from "@/components/FlagBadge";
import { Pagination } from "@/components/Pagination";
import { ReportPanel } from "@/components/ReportPanel";
import { ReviewQueue, type ReviewRow } from "@/components/ReviewQueue";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionsTable } from "@/components/SessionsTable";
import { SessionTabs } from "@/components/SessionTabs";
import { MOCK_ORG_ID, createMockClient } from "@/lib/api/mock";
import type { AuditEntry, Device, SessionDetail, SessionSummary } from "@/lib/api";
import { flagAssessmentText, flagCase } from "@/lib/flagAssessment";
import { messagesFor } from "@/lib/i18n/messages";
import { LocaleProvider } from "@/lib/i18n/provider";
import { navLinks } from "@/lib/nav";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), refresh: vi.fn() }) }));

const DEVANAGARI = /[ऀ-ॿ]/;
const ne = (ui: React.ReactElement) => render(<LocaleProvider locale="ne">{ui}</LocaleProvider>);

const summary = (over: Partial<SessionSummary> = {}): SessionSummary => ({
  id: "S1",
  participant_id: "p1",
  participant_token: "child-001",
  started_at: "2026-11-01T10:00:00.000Z",
  duration_sec: 125,
  quality_verdict: "optimal",
  attention_flag: "green",
  composite_score: 71.5,
  composite_confidence: 0.9,
  bundle_version: "1.0.0",
  device_id: "d1",
  reviewed: true,
  tracking_verdict: "good",
  tracking_accuracy_deg: 1.25,
  ...over,
});

describe("SessionsTable in ne", () => {
  it("replaces Yes/No, the flag, the verdicts and the headers, keeping Latin digits", () => {
    ne(<SessionsTable orgId="o" sessions={[summary(), summary({ id: "S2", reviewed: false, attention_flag: "red" })]} />);
    const rows = screen.getAllByRole("row").slice(1);
    const first = within(rows[0]!);
    expect(first.getByText("हो")).toBeInTheDocument();
    expect(within(rows[1]!).getByText("होइन")).toBeInTheDocument();
    expect(first.queryByText("Yes")).toBeNull();
    expect(first.getByText("हरियो")).toHaveClass("badge-green");
    expect(first.getByText(/71\.5/)).toBeInTheDocument();
    expect(first.getByText("2:05")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "सहभागी" })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/[०-९]/);
  });

  it("stays English without a provider", () => {
    render(<SessionsTable orgId="o" sessions={[summary()]} />);
    expect(screen.getByText("Yes")).toBeInTheDocument();
    expect(screen.getByText("Green")).toBeInTheDocument();
    expect(screen.getByText("Optimal")).toBeInTheDocument();
  });
});

describe("flag assessment text (I18N-4)", () => {
  const sent = { headline: "Looking matches the age reference", detail: "d" };

  it("maps the five English headlines and nothing else", () => {
    expect(flagCase("Looking matches the age reference")).toBe("matches");
    expect(flagCase("Recheck — looking time was unreliable")).toBe("unreliable");
    expect(flagCase("something from another version")).toBeNull();
  });

  it("FlagBadge shows flag_assessment.matches.headline in ne and the sent text for an unknown headline", () => {
    const { unmount } = ne(<FlagBadge flag="green" assessment={sent} />);
    expect(screen.getByText(flagAssessmentText("ne", sent).headline)).toBeInTheDocument();
    expect(screen.queryByText(sent.headline)).toBeNull();
    unmount();
    ne(<FlagBadge flag="green" assessment={{ headline: "Sent by a newer device" }} />);
    expect(screen.getByText("Sent by a newer device")).toBeInTheDocument();
  });

  it("ReportPanel shows the Nepali headline and detail, and falls back to the sent text", async () => {
    const api = createMockClient();
    const { items } = await api.listSessions(MOCK_ORG_ID, { limit: 5 });
    const session: SessionDetail = await api.getSession(items[0]!.id);
    const { unmount } = ne(<ReportPanel session={session} assessment={sent} />);
    const text = flagAssessmentText("ne", sent);
    expect(screen.getByText(text.headline)).toBeInTheDocument();
    expect(screen.getByText(text.detail!, { exact: false })).toBeInTheDocument();
    unmount();
    ne(<ReportPanel session={session} assessment={{ headline: "Unknown headline", detail: "Unknown detail" }} />);
    expect(screen.getByText("Unknown headline")).toBeInTheDocument();
    expect(screen.getByText("Unknown detail", { exact: false })).toBeInTheDocument();
  });
});

describe("other components in ne", () => {
  it("ReviewQueue: reasons, statuses and SLA units in Nepali with Latin digits", () => {
    const NOW = Date.parse("2026-11-01T12:00:00Z");
    const row: ReviewRow = {
      item: {
        id: "r1", org_id: "o", session_id: "S1", reason: "flag_red", status: "open",
        created_at: new Date(NOW - 3_600_000).toISOString(), assigned_to: null, decision: null, decided_by: null, decided_at: null,
      },
      session: { participant_token: "child-001", attention_flag: "red", quality_verdict: "degraded", started_at: new Date(NOW - 3_700_000).toISOString() },
    };
    ne(<ReviewQueue orgId="o" rows={[row]} canWrite now={NOW} filterStatus="open" triage={vi.fn()} />);
    const tr = screen.getAllByRole("row")[1]!;
    expect(within(tr).getByText("रातो फ्लाग")).toBeInTheDocument();
    expect(within(tr).getByText("खुला")).toBeInTheDocument();
    expect(within(tr).getByText("1घ 0मि")).toBeInTheDocument();
    expect(screen.getByRole("toolbar", { name: messagesFor("ne").app.review.toolbar })).toBeInTheDocument();
  });

  it("DevicesTable, AuditTable, Pagination, SessionTabs and SessionFilters", () => {
    const device = {
      id: "dev-0001", label: "iPad", platform: "ios", app_version: "1.0", config_bundle_version: "1.0.0",
      enrolled_at: "2026-10-01T10:00:00.000Z", last_seen_at: null, revoked_at: null,
    } as Device;
    const dev = ne(<DevicesTable devices={[device]} canRevoke revoke={vi.fn()} />);
    expect(screen.getByText("सक्रिय")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "रद्द गर्नुहोस्" })).toBeInTheDocument();
    dev.unmount();

    const entry = { id: "a1", ts: "2026-11-01T10:00:00.000Z", actor_type: "user", actor_id: "abcdefghij", action: "org.update", target_type: null, target_id: null, before: null, after: { a: 1 } } as unknown as AuditEntry;
    const aud = ne(<AuditTable entries={[entry]} />);
    expect(screen.getByText("प्रयोगकर्ता", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("देखाउनुहोस्")).toBeInTheDocument();
    aud.unmount();

    const pag = ne(<Pagination orgId="o" query={{}} nextCursor="c" shown={3} total={9} />);
    expect(screen.getByRole("navigation", { name: "पृष्ठ सङ्ख्या" })).toBeInTheDocument();
    expect(screen.getByText("जम्मा 9 मध्ये 3 सत्र देखाइँदै")).toBeInTheDocument();
    pag.unmount();

    const labels = messagesFor("ne").app.detail.tabs;
    const tabs = render(<SessionTabs basePath="/o/sessions/S" active="clips" labels={labels} label={messagesFor("ne").app.detail.tabsLabel} />);
    expect(screen.getByRole("link", { name: "क्लिपहरू" })).toBeInTheDocument();
    tabs.unmount();

    ne(<SessionFilters orgId="o" query={{}} />);
    expect(screen.getByRole("option", { name: "उत्तम" })).toBeInTheDocument();
  });

  it("navLinks takes its labels from the bundle", () => {
    const links = navLinks("/o", "owner", messagesFor("ne"));
    expect(links.find((l) => l.href === "/o/overview")?.text).toBe("सारांश");
    expect(links.find((l) => l.href === "/o/audit")?.text).toMatch(DEVANAGARI);
    expect(navLinks("/o", "owner").find((l) => l.href === "/o/overview")?.text).toBe("Overview");
  });
});
