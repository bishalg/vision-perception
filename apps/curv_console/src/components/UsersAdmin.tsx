"use client";

import { useActionState, useEffect, useRef, useState, useTransition, type FormEvent } from "react";
import { MAX_TOTP_RESET_REASON_CHARS, MIN_TOTP_RESET_REASON_CHARS, type OrgUser, type Role } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";
import type { Messages } from "@/lib/i18n/messages";
import { can, canEditUserRole } from "@/lib/permissions";


export type UserActionResult =
  | { ok: true; email: string }
  | { ok: false; slug: string | null; status: number; params?: string };

export type UserAction = (formData: FormData) => Promise<UserActionResult>;
export type UpdateUserFn = (userId: string, formData: FormData) => Promise<UserActionResult>;

/** Result of the TRS-1 reset action: a slug and status, never the submitted password, code or reason. */
export type TotpResetResult = { ok: true } | { ok: false; slug: string | null; status: number; retryAfter?: number };
export type ResetTotpFn = (userId: string, formData: FormData) => Promise<TotpResetResult>;

export function totpResetErrorText(r: Extract<TotpResetResult, { ok: false }>, tr: Messages["admin"]["users"]["totpReset"]): string {
  if (r.slug && tr.errors[r.slug]) return tr.errors[r.slug]!;
  if (r.status === 429) return tr.errors["rate-limited"]!;
  if (r.status === 403) return tr.errors.forbidden!;
  if (r.status === 404) return tr.errors["not-found"]!;
  if (r.status === 0) return tr.errors.unavailable!;
  return `${tr.failed} (status ${r.status})`;
}

/**
 * TRS-3 step-up dialog (UI-3). Like the sign dialog, the password and code inputs are uncontrolled: they live only in
 * the DOM, are read into a `FormData` at submit, and the form is reset before the request is awaited, so neither value
 * ever enters React state. The sentence that the user must enrol again is always shown.
 */
export function TotpResetDialog({
  user,
  reset,
  onDone,
  onClose,
}: {
  user: OrgUser;
  reset: ResetTotpFn;
  onDone: () => void;
  onClose: () => void;
}) {
  const tx = useText();
  const tr = tx.admin.users.totpReset;
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<Extract<TotpResetResult, { ok: false }> | null>(null);
  const first = useRef<HTMLTextAreaElement>(null);
  useEffect(() => first.current?.focus(), []);

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    // Keep the reason on screen if the password was wrong; only the secrets leave the DOM now.
    for (const name of ["password", "totp_code"]) {
      const el = form.elements.namedItem(name);
      if (el instanceof HTMLInputElement) el.value = "";
    }
    setPending(true);
    setFailure(null);
    let result: TotpResetResult;
    try {
      result = await reset(user.id, data);
    } catch {
      result = { ok: false, slug: null, status: 0 };
    }
    setPending(false);
    if (result.ok) onDone();
    else setFailure(result);
  }

  return (
    <div className="dialog-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={`totp-reset-${user.id}`}
        className="dialog panel"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !pending) onClose();
        }}
      >
        <h2 id={`totp-reset-${user.id}`}>{tr.title(user.email)}</h2>
        <p>{tr.intro}</p>
        <form onSubmit={onSubmit} autoComplete="off" className="stack">
          <label>
            {tr.reason}
            <textarea
              ref={first}
              name="reason"
              rows={2}
              required
              minLength={MIN_TOTP_RESET_REASON_CHARS}
              maxLength={MAX_TOTP_RESET_REASON_CHARS}
            />
          </label>
          <p className="muted">{tr.reasonHelp(MIN_TOTP_RESET_REASON_CHARS, MAX_TOTP_RESET_REASON_CHARS)}</p>
          <label>
            {tr.password}
            <input name="password" type="password" autoComplete="current-password" required />
          </label>
          <label>
            {tr.totp}
            <input name="totp_code" inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]*" maxLength={8} />
          </label>
          <p className="muted">{tr.secretNote}</p>
          {failure ? (
            <p role="alert" className="error">{tr.failed} {totpResetErrorText(failure, tr)}</p>
          ) : null}
          <div className="dialog-actions">
            <button type="submit" disabled={pending}>{pending ? tr.submitting : tr.submit}</button>
            <button type="button" className="secondary" onClick={onClose} disabled={pending}>{tr.cancel}</button>
          </div>
        </form>
      </div>
    </div>
  );
}

function errorText(r: Extract<UserActionResult, { ok: false }>, t: Messages["admin"]["users"]): string {
  const base = (r.slug && t.errors[r.slug]) || (r.status === 0 ? t.errors.unavailable : null);
  return [base, r.params].filter(Boolean).join(" ") || `status ${r.status}`;
}

function EditRow({ user, roles, update, onDone }: { user: OrgUser; roles: Role[]; update: UpdateUserFn; onDone: () => void }) {
  const tx = useText();
  const t = tx.admin.users;
  const [error, setError] = useState<string | null>(null);
  const [pending, startTransition] = useTransition();
  // Keep the current role selectable even when the actor could not grant it (the backend then rejects a change).
  const options = roles.includes(user.role) ? roles : [user.role, ...roles];
  return (
    <form
      className="confirm"
      aria-label={t.editLabel(user.email)}
      action={(fd) =>
        startTransition(async () => {
          const res = await update(user.id, fd);
          if (res.ok) onDone();
          else setError(`${t.updateFailed} ${errorText(res, t)}`);
        })
      }
    >
      <select name="role" defaultValue={user.role} aria-label={t.roleLabel}>
        {options.map((r) => (
          <option key={r} value={r}>{t.roles[r]}</option>
        ))}
      </select>
      <select name="status" defaultValue={user.status === "invited" ? "" : user.status} aria-label={t.statusLabel}>
        {user.status === "invited" ? <option value="">{t.statuses.invited}</option> : null}
        <option value="active">{t.enable}</option>
        <option value="disabled">{t.disable}</option>
      </select>
      <button type="submit" disabled={pending}>{t.save}</button>
      <button type="button" className="link-button" onClick={onDone}>{t.cancel}</button>
      {error ? <span role="alert" className="error">{error}</span> : null}
    </form>
  );
}

function Row({
  user,
  isSelf,
  canEdit,
  roles,
  update,
  showActions,
  resetTotp,
}: {
  user: OrgUser;
  isSelf: boolean;
  canEdit: boolean;
  roles: Role[];
  update: UpdateUserFn;
  showActions: boolean;
  /** Present when the actor may reset this user's two-step sign-in (not self, not a higher rank). */
  resetTotp?: ResetTotpFn;
}) {
  const tx = useText();
  const t = tx.admin.users;
  const c = tx.admin.users.cols;
  const tr = tx.admin.users.totpReset;
  const [editing, setEditing] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [resetDone, setResetDone] = useState(false);
  return (
    <tr>
      <td data-label={c.user}>
        {user.display_name ?? "-"}
        {isSelf ? <span className="muted"> ({t.you})</span> : null}
      </td>
      <td data-label={c.email}>{user.email}</td>
      <td data-label={c.role}>{t.roles[user.role]}</td>
      <td data-label={c.status}>
        <span className={`badge ${user.status === "active" ? "badge-green" : user.status === "invited" ? "badge-yellow" : "badge-unreliable"}`}>
          {t.statuses[user.status]}
        </span>
      </td>
      <td data-label={c.lastLogin}>{user.last_login_at ? formatDateTime(user.last_login_at) : t.never}</td>
      <td data-label={c.registration}>{user.professional_registration ?? "-"}</td>
      {showActions ? (
        <td data-label={c.actions}>
          {!canEdit || isSelf ? null : editing ? (
            <EditRow user={user} roles={roles} update={update} onDone={() => setEditing(false)} />
          ) : (
            <button type="button" className="secondary" onClick={() => setEditing(true)}>{t.edit}</button>
          )}{" "}
          {resetTotp && !editing ? (
            <button type="button" className="secondary" aria-label={tr.buttonLabel(user.email)} onClick={() => { setResetDone(false); setResetting(true); }}>
              {tr.button}
            </button>
          ) : null}
          {resetting && resetTotp ? (
            <TotpResetDialog
              user={user}
              reset={resetTotp}
              onDone={() => { setResetting(false); setResetDone(true); }}
              onClose={() => setResetting(false)}
            />
          ) : null}
          {resetDone ? <p role="status">{tr.done(user.email)}</p> : null}
        </td>
      ) : null}
    </tr>
  );
}

/** `actor` drives the rank rule only; `roles` are the roles the actor may grant. The backend decides. */
export function UsersTable({
  users,
  selfId,
  canManage,
  actor,
  roles,
  update,
  resetTotp,
}: {
  users: OrgUser[];
  selfId: string | null;
  canManage: boolean;
  actor: Role | undefined;
  roles: Role[];
  update: UpdateUserFn;
  /** TRS-3: the reset action, passed only to `user.totp_reset` roles. */
  resetTotp?: ResetTotpFn;
}) {
  const tx = useText();
  const t = tx.admin.users;
  const c = tx.admin.users.cols;
  if (users.length === 0) return <p className="muted">{t.empty}</p>;
  // Same rank rule as editing: never yourself, never a role above yours (the backend answers totp-reset-forbidden).
  const mayReset = (u: OrgUser) =>
    Boolean(resetTotp) && can(actor, "user.totp_reset") && u.id !== selfId && canEditUserRole(actor, u.role);
  return (
    <div className="table-wrap">
      <table className="data responsive">
        <thead>
          <tr>
            <th scope="col">{c.user}</th>
            <th scope="col">{c.email}</th>
            <th scope="col">{c.role}</th>
            <th scope="col">{c.status}</th>
            <th scope="col">{c.lastLogin}</th>
            <th scope="col">{c.registration}</th>
            {canManage ? <th scope="col">{c.actions}</th> : null}
          </tr>
        </thead>
        <tbody>
          {users.map((u) => (
            <Row
              key={u.id}
              user={u}
              isSelf={u.id === selfId}
              canEdit={canManage && canEditUserRole(actor, u.role)}
              roles={roles}
              update={update}
              showActions={canManage}
              resetTotp={mayReset(u) ? resetTotp : undefined}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

type CreateState = UserActionResult | null;

export function CreateUserForm({ create, roles }: { create: UserAction; roles: Role[] }) {
  const tx = useText();
  const t = tx.admin.users;
  const [state, action, pending] = useActionState<CreateState, FormData>((_prev, fd) => create(fd), null);
  const ct = t.create;
  return (
    <form action={action} className="stack panel" aria-label={ct.title}>
      <h2>{ct.title}</h2>
      <p className="muted">{ct.intro}</p>
      <label>
        {ct.email}
        <input name="email" type="email" required maxLength={254} autoComplete="off" />
      </label>
      <label>
        {ct.role}
        <select name="role" defaultValue={roles[roles.length - 1]} required>
          {roles.map((r) => (
            <option key={r} value={r}>{t.roles[r]}</option>
          ))}
        </select>
      </label>
      <label>
        {ct.password}
        <input name="password" type="password" required minLength={12} autoComplete="new-password" />
      </label>
      <label>
        {ct.displayName}
        <input name="display_name" maxLength={120} autoComplete="off" />
      </label>
      <label>
        {ct.registration}
        <input name="professional_registration" maxLength={60} autoComplete="off" />
      </label>
      <div>
        <button type="submit" disabled={pending}>{ct.submit}</button>
      </div>
      {state?.ok ? <p role="status">{ct.created(state.email)}</p> : null}
      {state && !state.ok ? (
        <p role="alert" className="error">{ct.failed} {errorText(state, t)}</p>
      ) : null}
    </form>
  );
}
