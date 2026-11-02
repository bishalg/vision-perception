import type { Metadata } from "next";
import Link from "next/link";
import { CreateUserForm, UsersTable } from "@/components/UsersAdmin";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { assignableRoles, can } from "@/lib/permissions";
import { adminText } from "@/messages/admin";
import { createUserAction, resetTotpAction, updateUserAction } from "./actions";

const t = adminText.users;

export const metadata: Metadata = { title: t.title };

export default async function UsersPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const role = await currentRole();
  if (!can(role, "user.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{adminText.common.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const [page, me] = await Promise.all([
    loadOrNotFound(() => api.listUsers(org, { cursor, limit: 50 })),
    loadOrNotFound(() => api.getMyProfile()),
  ]);
  const canManage = can(role, "user.manage");
  const roles = assignableRoles(role);
  const base = `/${encodeURIComponent(org)}/users`;
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <UsersTable
        users={page.items}
        selfId={me.id}
        canManage={canManage}
        actor={role}
        roles={roles}
        update={updateUserAction.bind(null, org)}
        resetTotp={can(role, "user.totp_reset") ? resetTotpAction.bind(null, org) : undefined}
      />
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{adminText.common.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{adminText.common.next}</Link>
        ) : null}
      </nav>
      {canManage && roles.length > 0 ? <CreateUserForm create={createUserAction.bind(null, org)} roles={roles} /> : null}
    </>
  );
}
