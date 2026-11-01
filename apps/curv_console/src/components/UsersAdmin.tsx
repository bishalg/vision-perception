"use client";

import { useActionState, useState, useTransition } from "react";
import type { OrgUser, Role } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { canEditUserRole } from "@/lib/permissions";
import { adminText } from "@/messages/admin";

const t = adminText.users;
const c = t.cols;

export type UserActionResult =
  | { ok: true; email: string }
  | { ok: false; slug: string | null; status: number; params?: string };

export type UserAction = (formData: FormData) => Promise<UserActionResult>;
export type UpdateUserFn = (userId: string, formData: FormData) => Promise<UserActionResult>;

function errorText(r: Extract<UserActionResult, { ok: false }>): string {
  const base = (r.slug && t.errors[r.slug]) || (r.status === 0 ? t.errors.unavailable : null);
  return [base, r.params].filter(Boolean).join(" ") || `status ${r.status}`;
}

function EditRow({ user, roles, update, onDone }: { user: OrgUser; roles: Role[]; update: UpdateUserFn; onDone: () => void }) {
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
          else setError(`${t.updateFailed} ${errorText(res)}`);
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
}: {
  user: OrgUser;
  isSelf: boolean;
  canEdit: boolean;
  roles: Role[];
  update: UpdateUserFn;
  showActions: boolean;
}) {
  const [editing, setEditing] = useState(false);
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
          )}
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
}: {
  users: OrgUser[];
  selfId: string | null;
  canManage: boolean;
  actor: Role | undefined;
  roles: Role[];
  update: UpdateUserFn;
}) {
  if (users.length === 0) return <p className="muted">{t.empty}</p>;
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
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

type CreateState = UserActionResult | null;

export function CreateUserForm({ create, roles }: { create: UserAction; roles: Role[] }) {
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
        <p role="alert" className="error">{ct.failed} {errorText(state)}</p>
      ) : null}
    </form>
  );
}
