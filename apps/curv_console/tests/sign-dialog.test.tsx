import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CosignDialog } from "@/components/clinical/CosignDialog";
import { ReviewPanel } from "@/components/clinical/ReviewPanel";
import { SignDialog } from "@/components/clinical/SignDialog";
import type { ReviewActions, ReviewResult } from "@/components/clinical/types";
import type { ClinicalReview } from "@/lib/api/clinical";

const HASH = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

function review(over: Partial<ClinicalReview> = {}, current: Partial<ClinicalReview["current"]> = {}): ClinicalReview {
  return {
    id: "rev-1", org_id: "o", session_id: "s", state: "in_progress", assigned_to: "u", cosign_required: false,
    row_version: 4, content_hash: HASH, signatures: [], created_at: "2026-11-01T00:00:00Z", updated_at: "2026-11-01T00:00:00Z",
    current: {
      version_no: 1, status: "draft", created_at: "2026-11-01T00:00:00Z", disposition: "monitor",
      impression: "Synthetic impression", recommendations: "Synthetic plan", ...current,
    },
    ...over,
  };
}

const ok = (r: ClinicalReview): ReviewResult => ({ ok: true, review: r });
const fail = (slug: string | null, status: number): ReviewResult => ({ ok: false, slug, status });

function dialogProps(submit: (fd: FormData) => Promise<ReviewResult>) {
  return { review: review(), submit, onDone: vi.fn(), onClose: vi.fn(), onReload: vi.fn() };
}

describe("SignDialog", () => {
  it("shows the meaning statement and the content-hash prefix from the API", () => {
    render(<SignDialog {...dialogProps(vi.fn())} />);
    expect(screen.getByRole("dialog", { name: "Sign clinical review" })).toBeInTheDocument();
    expect(screen.getByText("I have reviewed this observation record and approve this review.")).toBeInTheDocument();
    expect(screen.getByTestId("hash-prefix")).toHaveTextContent(HASH.slice(0, 12));
    expect(screen.getByTestId("hash-prefix").textContent).not.toContain(HASH.slice(0, 13));
  });

  it("asks for password and optional TOTP, and needs the acknowledgement", () => {
    render(<SignDialog {...dialogProps(vi.fn())} />);
    expect(screen.getByLabelText("Your password")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText(/Authenticator code/)).toBeInTheDocument();
    expect(screen.getByLabelText(/I confirm the statement/)).toBeRequired();
  });

  it("hands the password to the action once, clears the field before the answer, and keeps it out of the DOM and state", async () => {
    const user = userEvent.setup();
    let release!: (r: ReviewResult) => void;
    const submit = vi.fn((fd: FormData) => {
      void fd;
      return new Promise<ReviewResult>((res) => (release = res));
    });
    const props = dialogProps(submit);
    const { container } = render(<SignDialog {...props} />);
    await user.type(screen.getByLabelText("Your password"), "s3cret-pass");
    await user.type(screen.getByLabelText(/Authenticator code/), "123456");
    await user.click(screen.getByLabelText(/I confirm the statement/));
    await user.click(screen.getByRole("button", { name: "Sign review" }));

    expect(submit).toHaveBeenCalledTimes(1);
    const fd = submit.mock.calls[0]![0];
    expect(fd.get("password")).toBe("s3cret-pass");
    expect(fd.get("totp_code")).toBe("123456");
    expect(fd.get("content_hash")).toBe(HASH); // the API's hash, echoed unchanged
    expect(fd.get("meaning_ack")).toBe("true");
    // While the request is in flight the secret is already gone from the page.
    expect(screen.getByLabelText("Your password")).toHaveValue("");
    expect(container.innerHTML).not.toContain("s3cret-pass");

    release(ok(review({ state: "final" })));
    await waitFor(() => expect(props.onDone).toHaveBeenCalled());
    expect(container.innerHTML).not.toContain("s3cret-pass");
  });

  it("shows a reload prompt on 409 content-changed and reloads on request", async () => {
    const user = userEvent.setup();
    const props = dialogProps(vi.fn().mockResolvedValue(fail("content-changed", 409)));
    render(<SignDialog {...props} />);
    await user.type(screen.getByLabelText("Your password"), "pw");
    await user.click(screen.getByLabelText(/I confirm the statement/));
    await user.click(screen.getByRole("button", { name: "Sign review" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/changed after you opened it/);
    await user.click(screen.getByRole("button", { name: "Reload review" }));
    expect(props.onReload).toHaveBeenCalled();
    expect(props.onDone).not.toHaveBeenCalled();
  });

  it.each([
    ["reauthentication-failed", 401, /Password or authenticator code not accepted/],
    ["signer-profile-incomplete", 409, /display name in Clinical settings/],
    ["clinical-crypto-unavailable", 503, /temporarily unavailable/],
  ])("explains %s clearly", async (slug, status, text) => {
    const user = userEvent.setup();
    render(<SignDialog {...dialogProps(vi.fn().mockResolvedValue(fail(slug, status)))} />);
    await user.type(screen.getByLabelText("Your password"), "pw");
    await user.click(screen.getByLabelText(/I confirm the statement/));
    await user.click(screen.getByRole("button", { name: "Sign review" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
    expect(screen.queryByRole("button", { name: "Reload review" })).not.toBeInTheDocument();
  });

  it("Escape closes the dialog", async () => {
    const user = userEvent.setup();
    const props = dialogProps(vi.fn());
    render(<SignDialog {...props} />);
    await user.keyboard("{Escape}");
    expect(props.onClose).toHaveBeenCalled();
  });
});

describe("CosignDialog", () => {
  it("is the same step-up with cosign wording and surfaces cosigner-must-differ", async () => {
    const user = userEvent.setup();
    render(<CosignDialog {...dialogProps(vi.fn().mockResolvedValue(fail("cosigner-must-differ", 409)))} />);
    expect(screen.getByRole("dialog", { name: "Cosign clinical review" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("Your password"), "pw");
    await user.click(screen.getByLabelText(/I confirm the statement/));
    await user.click(screen.getByRole("button", { name: "Cosign review" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("A different clinician must cosign");
  });
});

describe("ReviewPanel", () => {
  function actions(over: Partial<ReviewActions> = {}): ReviewActions {
    const unused = vi.fn().mockResolvedValue(fail(null, 500));
    return { claim: unused, save: unused, sign: unused, cosign: unused, amend: unused, reload: unused, ...over };
  }
  const base = { canWrite: true, canSign: true, canCosign: true, canContent: true };

  it("offers Claim when no review exists, and claims through the action", async () => {
    const user = userEvent.setup();
    const claim = vi.fn().mockResolvedValue(ok(review({ state: "open" }, { disposition: null, impression: null, recommendations: null })));
    render(<ReviewPanel review={null} {...base} actions={actions({ claim })} />);
    await user.click(screen.getByRole("button", { name: "Claim review" }));
    expect(claim).toHaveBeenCalled();
    expect(await screen.findByLabelText("Disposition")).toBeEnabled();
  });

  it("does not pre-fill the disposition (SIGN-2)", () => {
    render(<ReviewPanel review={review({ state: "open" }, { disposition: null, impression: null, recommendations: null })} {...base} actions={actions()} />);
    expect(screen.getByLabelText("Disposition")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Sign" })).toBeDisabled();
  });

  it("saves a draft with the review's row_version (If-Match) and adopts the returned review", async () => {
    const user = userEvent.setup();
    const saved = review({ row_version: 5, content_hash: "ab".repeat(32) }, { impression: "Edited" });
    const save = vi.fn().mockResolvedValue(ok(saved));
    render(<ReviewPanel review={review()} {...base} actions={actions({ save })} />);
    const box = screen.getByLabelText("Impression");
    await user.clear(box);
    await user.type(box, "Edited");
    expect(screen.getByRole("button", { name: "Sign" })).toBeDisabled(); // unsaved changes block signing
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(save).toHaveBeenCalledWith("rev-1", 4, { disposition: "monitor", impression: "Edited", recommendations: "Synthetic plan" });
    expect(await screen.findByText("Draft saved.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign" })).toBeEnabled();
  });

  it("offers a reload after 412 stale-version", async () => {
    const user = userEvent.setup();
    const save = vi.fn().mockResolvedValue(fail("stale-version", 412));
    const reload = vi.fn().mockResolvedValue(ok(review({ row_version: 9 }, { impression: "Colleague's text" })));
    render(<ReviewPanel review={review()} {...base} actions={actions({ save, reload })} />);
    await user.type(screen.getByLabelText("Impression"), " more");
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/Someone else saved/);
    await user.click(screen.getByRole("button", { name: "Reload review" }));
    expect(reload).toHaveBeenCalledWith("rev-1");
    await waitFor(() => expect(screen.getByLabelText("Impression")).toHaveValue("Colleague's text"));
  });

  it("opens the sign dialog from a saved draft and ends Final after a successful signature", async () => {
    const user = userEvent.setup();
    const sign = vi.fn().mockResolvedValue(
      ok(review({ state: "final", signatures: [{ id: "g1", version_no: 1, role: "sign", signer_id: "u", signer_display_name: "Dr. Demo", signer_registration: "NMC 1", meaning: "m", content_hash: HASH, signed_at: "2026-11-01T01:00:00Z" }] }, { status: "signed" })),
    );
    render(<ReviewPanel review={review()} {...base} actions={actions({ sign })} />);
    await user.click(screen.getByRole("button", { name: "Sign" }));
    await user.type(screen.getByLabelText("Your password"), "pw");
    await user.click(screen.getByLabelText(/I confirm the statement/));
    await user.click(screen.getByRole("button", { name: "Sign review" }));
    await waitFor(() => expect(screen.getByTestId("review-state")).toHaveTextContent("Final"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText(/Dr\. Demo \(NMC 1\)/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Amend" })).toBeInTheDocument();
  });

  it("amend asks for a reason of at least 10 characters", async () => {
    const user = userEvent.setup();
    const amend = vi.fn().mockResolvedValue(ok(review({ state: "amending" }, { version_no: 2, amend_reason: "Fixed a typo in the plan" })));
    render(<ReviewPanel review={review({ state: "final" }, { status: "signed" })} {...base} actions={actions({ amend })} />);
    await user.click(screen.getByRole("button", { name: "Amend" }));
    const start = screen.getByRole("button", { name: "Start amendment" });
    expect(start).toBeDisabled();
    await user.type(screen.getByLabelText(/Reason for amendment/), "Fixed a typo in the plan");
    await user.click(start);
    expect(amend).toHaveBeenCalledWith("rev-1", "Fixed a typo in the plan");
    await waitFor(() => expect(screen.getByTestId("review-state")).toHaveTextContent("Amending"));
  });

  it("awaiting_cosign shows Cosign to a cosigner and no editing", () => {
    render(<ReviewPanel review={review({ state: "awaiting_cosign", cosign_required: true }, { status: "signed" })} {...base} actions={actions()} />);
    expect(screen.getByRole("button", { name: "Cosign" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Save draft" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Disposition")).toBeDisabled();
  });

  it("hides impression and recommendations from a role without content-read, and offers no write controls", () => {
    const hidden = review({ state: "in_progress" }, { impression: null, recommendations: null });
    render(<ReviewPanel review={hidden} canWrite={false} canSign={false} canCosign={false} canContent={false} actions={actions()} />);
    expect(screen.queryByLabelText("Impression")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Recommendations")).not.toBeInTheDocument();
    expect(screen.getByText(/hidden for your role/)).toBeInTheDocument();
    for (const name of ["Save draft", "Sign", "Cosign", "Amend", "Claim review"]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }
  });

  it("does not render clinical text for a role without content-read even if the payload carried it", () => {
    render(<ReviewPanel review={review()} canWrite={false} canSign={false} canCosign={false} canContent={false} actions={actions()} />);
    expect(screen.queryByText("Synthetic impression")).not.toBeInTheDocument();
    expect(document.body.innerHTML).not.toContain("Synthetic impression");
  });
});
