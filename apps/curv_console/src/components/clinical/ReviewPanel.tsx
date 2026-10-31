"use client";

import { useState } from "react";
import { DISPOSITIONS, dispositionSchema, type ClinicalReview } from "@/lib/api/clinical";
import { formatDateTime } from "@/lib/format";
import { clinicalErrorMessage, clinicalText, hashPrefix } from "@/messages/clinical";
import { AmendDialog } from "./AmendDialog";
import { CosignDialog } from "./CosignDialog";
import { SignDialog } from "./SignDialog";
import type { ResultFailure, ReviewActions, ReviewResult } from "./types";

const t = clinicalText.review;
const MAX_TEXT = 8000;
const EDITABLE = ["open", "in_progress", "amending"] as const;

export interface ReviewPanelProps {
  review: ClinicalReview | null;
  /** Role flags from `can()`; the backend enforces them again. */
  canWrite: boolean;
  canSign: boolean;
  canCosign: boolean;
  canContent: boolean;
  actions: ReviewActions;
}

/** The "Clinical review" tab body. Remount it (key) when the server hands over a different review. */
export function ReviewPanel(props: ReviewPanelProps) {
  const { canWrite, canSign, canCosign, canContent, actions } = props;
  const [review, setReview] = useState<ClinicalReview | null>(props.review);
  const [disposition, setDisposition] = useState<string>(props.review?.current.disposition ?? "");
  const [impression, setImpression] = useState(props.review?.current.impression ?? "");
  const [recommendations, setRecommendations] = useState(props.review?.current.recommendations ?? "");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<ResultFailure | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dialog, setDialog] = useState<"sign" | "cosign" | "amend" | null>(null);

  function adopt(next: ClinicalReview) {
    setReview(next);
    setDisposition(next.current.disposition ?? "");
    setImpression(next.current.impression ?? "");
    setRecommendations(next.current.recommendations ?? "");
    setFailure(null);
  }

  async function run(call: () => Promise<ReviewResult>, done?: string) {
    setBusy(true);
    setFailure(null);
    setNotice(null);
    let result: ReviewResult;
    try {
      result = await call();
    } catch {
      result = { ok: false, slug: null, status: 0 };
    }
    setBusy(false);
    if (result.ok) {
      adopt(result.review);
      if (done) setNotice(done);
    } else {
      setFailure({ slug: result.slug, status: result.status });
    }
  }

  async function reload() {
    if (!review) return;
    setDialog(null);
    await run(() => actions.reload(review.id));
  }

  if (!review) {
    return (
      <section className="panel" aria-label={t.title}>
        <p>{t.none}</p>
        {canWrite ? (
          <button type="button" disabled={busy} onClick={() => run(actions.claim)}>
            {busy ? t.claiming : t.claim}
          </button>
        ) : (
          <p className="muted">{t.readOnly}</p>
        )}
        <Failure failure={failure} onReload={null} />
      </section>
    );
  }

  const editable = canWrite && (EDITABLE as readonly string[]).includes(review.state);
  const showText = canContent;
  const dirty =
    disposition !== (review.current.disposition ?? "") ||
    (showText && (impression !== (review.current.impression ?? "") || recommendations !== (review.current.recommendations ?? "")));
  const parsedDisposition = dispositionSchema.safeParse(disposition);
  const canOpenSign = canSign && editable && !dirty && parsedDisposition.success;
  const canOpenCosign = canCosign && review.state === "awaiting_cosign";
  const canOpenAmend = canWrite && (review.state === "final" || review.state === "awaiting_cosign");

  return (
    <section className="panel review-panel" aria-label={t.title}>
      <dl className="facts">
        <div>
          <dt>{t.state}</dt>
          <dd>
            <span className="badge" data-testid="review-state">{t.states[review.state]}</span>
          </dd>
        </div>
        <div>
          <dt>{t.version(review.current.version_no)}</dt>
          <dd>{review.current.status}</dd>
        </div>
        <div>
          <dt>{t.contentHash}</dt>
          <dd>
            <code>{hashPrefix(review.content_hash)}</code>
          </dd>
        </div>
      </dl>
      {review.cosign_required ? <p className="muted">{t.cosignRequired}</p> : null}
      {review.current.amend_reason && showText ? (
        <p>
          <strong>{t.amendReason}:</strong> {review.current.amend_reason}
        </p>
      ) : null}
      {!showText ? <p className="muted">{t.contentHidden}</p> : null}
      {!canWrite ? <p className="muted">{t.readOnly}</p> : null}

      <form
        className="stack"
        aria-label={t.title}
        onSubmit={(e) => {
          e.preventDefault();
          if (!editable) return;
          const patch = {
            disposition: parsedDisposition.success ? parsedDisposition.data : null,
            ...(showText ? { impression, recommendations } : {}),
          };
          void run(() => actions.save(review.id, review.row_version, patch), t.saved);
        }}
      >
        <label>
          {t.disposition}
          <select
            name="disposition"
            value={disposition}
            disabled={!editable || busy}
            onChange={(e) => setDisposition(e.target.value)}
          >
            <option value="">{t.dispositionPick}</option>
            {DISPOSITIONS.map((d) => (
              <option key={d} value={d}>
                {t.dispositions[d]}
              </option>
            ))}
          </select>
        </label>
        {showText ? (
          <>
            <label>
              {t.impression}
              <textarea
                name="impression"
                rows={6}
                maxLength={MAX_TEXT}
                value={impression}
                readOnly={!editable}
                onChange={(e) => setImpression(e.target.value)}
              />
            </label>
            <p className="muted">{t.limit(impression.length, MAX_TEXT)}</p>
            <label>
              {t.recommendations}
              <textarea
                name="recommendations"
                rows={4}
                maxLength={MAX_TEXT}
                value={recommendations}
                readOnly={!editable}
                onChange={(e) => setRecommendations(e.target.value)}
              />
            </label>
            <p className="muted">{t.limit(recommendations.length, MAX_TEXT)}</p>
          </>
        ) : null}
        <div className="toolbar">
          {editable ? (
            <button type="submit" disabled={busy || !dirty}>
              {busy ? t.saving : t.saveDraft}
            </button>
          ) : null}
          {canSign && editable ? (
            <button type="button" disabled={busy || !canOpenSign} onClick={() => setDialog("sign")}>
              {t.sign}
            </button>
          ) : null}
          {canOpenCosign ? (
            <button type="button" disabled={busy} onClick={() => setDialog("cosign")}>
              {t.cosign}
            </button>
          ) : null}
          {canOpenAmend ? (
            <button type="button" className="secondary" disabled={busy} onClick={() => setDialog("amend")}>
              {t.amend}
            </button>
          ) : null}
        </div>
        {editable && canSign && dirty ? <p className="muted">{t.unsaved}</p> : null}
        {editable && canSign && !dirty && !parsedDisposition.success ? <p className="muted">{t.needDisposition}</p> : null}
      </form>

      <div role="status" aria-live="polite">
        {notice}
      </div>
      <Failure failure={failure} onReload={failure?.slug && clinicalText.reloadSlugs.includes(failure.slug) ? reload : null} />

      <h3>{t.signatures}</h3>
      {review.signatures.length === 0 ? (
        <p className="muted">{t.noSignatures}</p>
      ) : (
        <ul className="signatures">
          {review.signatures.map((s) => (
            <li key={s.id}>
              <strong>{t.roleLabel[s.role]}</strong> {t.signedBy(s.signer_display_name, s.signer_registration)},{" "}
              {formatDateTime(s.signed_at)}, v{s.version_no}, <code>{hashPrefix(s.content_hash)}</code>
              <br />
              <span className="muted">{s.meaning}</span>
            </li>
          ))}
        </ul>
      )}

      {dialog === "sign" ? (
        <SignDialog
          review={review}
          submit={(fd) => actions.sign(review.id, fd)}
          onDone={(next) => {
            adopt(next);
            setDialog(null);
            setNotice(null);
          }}
          onClose={() => setDialog(null)}
          onReload={reload}
        />
      ) : null}
      {dialog === "cosign" ? (
        <CosignDialog
          review={review}
          submit={(fd) => actions.cosign(review.id, fd)}
          onDone={(next) => {
            adopt(next);
            setDialog(null);
          }}
          onClose={() => setDialog(null)}
          onReload={reload}
        />
      ) : null}
      {dialog === "amend" ? (
        <AmendDialog
          submit={(reason) => actions.amend(review.id, reason)}
          onDone={(next) => {
            adopt(next);
            setDialog(null);
          }}
          onClose={() => setDialog(null)}
        />
      ) : null}
    </section>
  );
}

function Failure({ failure, onReload }: { failure: ResultFailure | null; onReload: (() => void) | null }) {
  if (!failure) return null;
  return (
    <div role="alert" className="error">
      <p>{clinicalErrorMessage(failure.slug, failure.status)}</p>
      {onReload ? (
        <button type="button" onClick={onReload}>
          {t.reload}
        </button>
      ) : null}
    </div>
  );
}
