import type { Metadata } from "next";
import Link from "next/link";
import { DevicesTable } from "@/components/DevicesTable";
import { EnrolCodePanel } from "@/components/EnrolCodePanel";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { en } from "@/messages/en";
import { createEnrolCodeAction, revokeDeviceAction } from "./actions";

export const metadata: Metadata = { title: en.devices.title };

export default async function DevicesPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const role = await currentRole();
  const page = await loadOrNotFound(() => api.listDevices(org, { cursor, limit: 50 }));
  const base = `/${encodeURIComponent(org)}/devices`;
  return (
    <>
      <h1>{en.devices.title}</h1>
      <EnrolCodePanel create={createEnrolCodeAction.bind(null, org)} canCreate={can(role, "enrol.create")} />
      <DevicesTable devices={page.items} canRevoke={can(role, "device.revoke")} revoke={revokeDeviceAction.bind(null, org)} />
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{en.devices.first}</Link> : null}
        {page.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: page.next_cursor })}`}>{en.devices.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
