/** WP-B.9: erasure requests, retention sweep and dataset exports. Wire format, mock rules, components and RBAC. */
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ExportForm } from "@/components/exports/ExportForm";
import { ErasureAction } from "@/components/privacy/ErasureAction";
import { ErasureTable } from "@/components/privacy/ErasureTable";
import { RetentionPanel } from "@/components/privacy/RetentionPanel";
import { ParticipantsTable } from "@/components/ParticipantsTable";
import { ApiError, ROLES } from "@/lib/api";
import { createLiveClient } from "@/lib/api/live";
import { createMockClient, MOCK_ORG_ID } from "@/lib/api/mock";
import type { ErasureRequest } from "@/lib/api/privacy";
import { navItems } from "@/lib/nav";
import { can } from "@/lib/permissions";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

const req = (over: Partial<ErasureRequest> = {}): ErasureRequest => ({
  id: "r1", org_id: "o", participant_id: "p1", participant_token: "child-001", status: "completed", scope: "all",
  requested_by_type: "user", requested_at: "2026-10-30T09:00:00.000Z", executed_at: null, completed_at: null,
  hold_reason: null, outcome: null, ...over,
});

describe("RBAC", () => {
  it("participant.erase is owner, admin and superadmin (the backend does not fence the superadmin)", () => {
    expect(ROLES.filter((r) => can(r, "participant.erase")).sort()).toEqual(["admin", "owner", "superadmin"]);
    const hrefs = (r: Parameters<typeof navItems>[1]) => navItems("/o", r).map((l) => l.href);
    expect(hrefs("owner")).toEqual(expect.arrayContaining(["/o/data/erasure", "/o/data/retention"]));
    expect(hrefs("clinician")).not.toContain("/o/data/erasure");
    expect(hrefs("researcher")).not.toContain("/o/data/retention");
  });
});

describe("live client wire format", () => {
  function client(response: Response) {
    const f = vi.fn().mockResolvedValue(response);
    return { f, api: createLiveClient({ baseUrl: "https://api.test", accessToken: "tok", fetchImpl: f as unknown as typeof fetch }) };
  }

  it("lists erasure requests with cursor and limit", async () => {
    const { f, api } = client(json({ items: [req()], next_cursor: null }));
    await api.listErasureRequests("o 1", { cursor: "c", limit: 50 });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o%201/erasure-requests?cursor=c&limit=50");
  });

  it("files an erasure with POST and no body; 200 duplicate parses like 202", async () => {
    const { f, api } = client(json(req({ status: "pending", duplicate: true }), 200));
    const r = await api.requestParticipantErasure("o", "p/1");
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/participants/p%2F1/erasure");
    expect(f.mock.calls[0]![1].method).toBe("POST");
    expect(f.mock.calls[0]![1].body).toBeUndefined();
    expect(r.duplicate).toBe(true);
  });

  it("starts a sweep with dry_run in the body and reads a job", async () => {
    const { f, api } = client(json({ job_id: "j1", org_id: "o", dry_run: true, status: "queued", created_at: "2026-10-31T00:00:00Z" }, 202));
    await api.startRetentionSweep("o", { dry_run: true });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/orgs/o/retention-sweep");
    expect(JSON.parse(f.mock.calls[0]![1].body)).toEqual({ dry_run: true });
    const j = client(json({ id: "j1", kind: "retention_sweep", status: "running", result: null, attempts: 1 }));
    expect((await j.api.getJob("j1")).status).toBe("running");
    expect(j.f.mock.calls[0]![0]).toBe("https://api.test/v1/jobs/j1");
  });

  it("creates a dataset export on /jobs/export-dataset", async () => {
    const { f, api } = client(json({
      id: "e1", org_id: "o", kind: "bulk", format: "dataset_jsonl", mode: "deidentified", purpose: "research", status: "queued",
      filters: {}, reason: "A long enough reason", destination_ids: [], created_at: "2026-10-31T00:00:00Z", excluded: {}, include_labels: true,
    }, 202));
    const e = await api.createDatasetExport({ format: "jsonl", reason: "A long enough reason", include_labels: true });
    expect(f.mock.calls[0]![0]).toBe("https://api.test/v1/jobs/export-dataset");
    expect(JSON.parse(f.mock.calls[0]![1].body)).toEqual({ format: "jsonl", reason: "A long enough reason", include_labels: true });
    expect(e.include_labels).toBe(true);
  });
});

describe("mock client", () => {
  it("files an erasure, answers duplicate for an open one and marks the participant withdrawn", async () => {
    const api = createMockClient();
    const p = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => !x.withdrawn_at)!;
    const first = await api.requestParticipantErasure(MOCK_ORG_ID, p.id);
    expect([first.status, first.duplicate]).toEqual(["pending", false]);
    expect((await api.requestParticipantErasure(MOCK_ORG_ID, p.id)).duplicate).toBe(true);
    const after = (await api.listParticipants(MOCK_ORG_ID, { limit: 100 })).items.find((x) => x.id === p.id)!;
    expect(after.withdrawn_at).toBeTruthy();
    const page = await api.listErasureRequests(MOCK_ORG_ID);
    expect(page.items.find((r) => r.id === first.id)!.status).toBe("pending");
    const later = await api.listErasureRequests(MOCK_ORG_ID);
    expect(["completed", "held"]).toContain(later.items.find((r) => r.id === first.id)!.status);
  });

  it("404s an unknown participant or job and rejects another org", async () => {
    const api = createMockClient();
    await expect(api.requestParticipantErasure(MOCK_ORG_ID, "nope")).rejects.toMatchObject({ status: 404 });
    await expect(api.getJob("nope")).rejects.toBeInstanceOf(ApiError);
    await expect(api.listErasureRequests("other")).rejects.toMatchObject({ status: 404 });
  });

  it("a dry run counts and a real sweep purges, so the next preview counts zero", async () => {
    const api = createMockClient();
    const run = async (dry_run: boolean) => {
      const job = await api.startRetentionSweep(MOCK_ORG_ID, { dry_run });
      expect((await api.getJob(job.job_id)).status).toBe("running");
      return (await api.getJob(job.job_id)).result as { orgs: { frames_purged: number }[] };
    };
    expect((await run(true)).orgs[0]!.frames_purged).toBeGreaterThan(0);
    await run(false);
    expect((await run(true)).orgs[0]!.frames_purged).toBe(0);
  });

  it("refuses an identified dataset on the generic export route and validates the dataset reason", async () => {
    const api = createMockClient();
    await expect(
      api.createExport(MOCK_ORG_ID, { format: "dataset_jsonl", mode: "identified", purpose: "research", reason: "A long enough reason" }),
    ).rejects.toMatchObject({ status: 400 });
    await expect(api.createDatasetExport({ format: "jsonl", reason: "short" })).rejects.toMatchObject({ status: 400 });
    const e = await api.createDatasetExport({ format: "jsonl", reason: "A long enough reason" });
    expect(e.include_labels).toBe(true);
  });
});

describe("components", () => {
  it("explains held as kept under clinical record hold and shows counts only", () => {
    render(
      <ErasureTable
        items={[
          req({ status: "held", hold_reason: "clinical_record", outcome: {
            sessions_deleted: 2, sessions_held: 3, care_events_deleted: 0, session_blobs_deleted: 0,
            export_artifacts_deleted: 0, clinical_drafts_deleted: 1, patient_ref_deleted: false } }),
          req({ id: "r2", status: "pending", participant_token: "child-002" }),
        ]}
      />,
    );
    expect(screen.getByText("Held")).toBeInTheDocument();
    expect(screen.getByText("Kept under clinical record hold")).toBeInTheDocument();
    expect(screen.getByText(/2 sessions deleted, 3 sessions held/)).toBeInTheDocument();
    expect(screen.getByText("Not run yet")).toBeInTheDocument();
  });

  it("ErasureAction confirms in the page and notes a duplicate", async () => {
    const user = userEvent.setup();
    const request = vi.fn().mockResolvedValue({ ok: true, duplicate: true });
    render(<ErasureAction token="child-001" request={request} requestsHref="/o/data/erasure" />);
    await user.click(screen.getByRole("button", { name: /Request erasure/ }));
    expect(request).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog")).toHaveTextContent("child-001");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await user.click(screen.getByRole("button", { name: /Request erasure/ }));
    await user.click(screen.getByRole("button", { name: "Yes, request erasure" }));
    expect(await screen.findByText(/already open/)).toBeInTheDocument();
    expect(request).toHaveBeenCalledTimes(1);
  });

  it("ErasureAction shows a failure", async () => {
    const user = userEvent.setup();
    render(<ErasureAction token="t" request={async () => ({ ok: false, slug: "forbidden", status: 403 })} requestsHref="/o/data/erasure" />);
    await user.click(screen.getByRole("button", { name: /Request erasure/ }));
    await user.click(screen.getByRole("button", { name: "Yes, request erasure" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Your role cannot do this.");
  });

  it("ParticipantsTable shows the erasure action only when given `erase`", () => {
    const p = { id: "p1", org_id: "o", participant_token: "child-001", session_count: 1, withdrawn_at: null };
    const { unmount } = render(<ParticipantsTable participants={[p]} />);
    expect(screen.queryByRole("button", { name: /Request erasure/ })).toBeNull();
    unmount();
    render(<ParticipantsTable participants={[p]} orgId="o" erase={async () => ({ ok: true, duplicate: false })} />);
    expect(screen.getByRole("button", { name: "Request erasure child-001" })).toBeInTheDocument();
  });

  it("RetentionPanel polls the job and shows counts; real sweep needs confirmation", async () => {
    const user = userEvent.setup();
    const start = vi.fn().mockResolvedValue({ ok: true, jobId: "j1" });
    const poll = vi
      .fn()
      .mockResolvedValueOnce({ ok: true, poll: { state: "pending", status: "running" } })
      .mockResolvedValue({
        ok: true,
        poll: { state: "done", result: { dry_run: true, orgs: [{ org_id: "o", retention_months: 24, cutoff: "2024-10-31T00:00:00Z", frames_purged: 17 }] } },
      });
    render(<RetentionPanel start={start} poll={poll} pollMs={1} />);
    await user.click(screen.getByRole("button", { name: "Preview (dry run)" }));
    expect(start).toHaveBeenCalledWith(true);
    expect(await screen.findByText("17 would be deleted")).toBeInTheDocument();
    expect(poll).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Run sweep" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("17 sessions");
    expect(start).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Yes, run sweep" }));
    await waitFor(() => expect(start).toHaveBeenLastCalledWith(false));
  });

  it("RetentionPanel reports a failed job and a failed start", async () => {
    const user = userEvent.setup();
    const poll = vi.fn().mockResolvedValue({ ok: true, poll: { state: "failed" } });
    const { unmount } = render(<RetentionPanel start={async () => ({ ok: true, jobId: "j" })} poll={poll} pollMs={1} />);
    await user.click(screen.getByRole("button", { name: "Preview (dry run)" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The job failed.");
    unmount();
    render(<RetentionPanel start={async () => ({ ok: false, slug: "forbidden", status: 403 })} poll={poll} pollMs={1} />);
    await user.click(screen.getByRole("button", { name: "Preview (dry run)" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Your role cannot do this.");
  });

  it("ExportForm: dataset formats lock the mode and purpose and offer labels", async () => {
    const user = userEvent.setup();
    render(<ExportForm create={async () => ({ ok: true, id: "e" })} canCreate canIdentified destinations={[]} orgId="o" />);
    const mode = screen.getByLabelText("Mode") as HTMLSelectElement;
    expect([...mode.options].map((o) => o.value)).toEqual(["identified", "deidentified"]);
    expect(screen.queryByLabelText("Include reviewer labels")).toBeNull();
    await user.selectOptions(screen.getByLabelText("Format"), "dataset_jsonl");
    const locked = screen.getByLabelText("Mode") as HTMLSelectElement;
    expect([...locked.options].map((o) => o.value)).toEqual(["deidentified"]);
    expect(locked).toBeDisabled();
    expect(screen.getByLabelText("Purpose")).toBeDisabled();
    expect(screen.getByLabelText("Include reviewer labels")).toBeChecked();
    expect(screen.getByRole("option", { name: "Research dataset (Parquet)" })).toBeInTheDocument();
  });
});
