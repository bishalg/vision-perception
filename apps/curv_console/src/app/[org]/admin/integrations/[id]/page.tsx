import type { Metadata } from "next";
import Link from "next/link";
import { DeliveriesTable } from "@/components/exports/DeliveriesTable";
import { IntegrationForm } from "@/components/exports/IntegrationForm";
import { DeleteIntegrationPanel, SecretPanel, SshKeyPanel, TestPanel } from "@/components/exports/IntegrationPanels";
import { TestBadge } from "@/components/exports/IntegrationsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { parseCursor, toSearch } from "@/lib/listQuery";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { getText } from "@/lib/i18n/server";
import {
  deleteIntegrationAction,
  generateSshKeyAction,
  putSecretAction,
  testIntegrationAction,
  updateIntegrationAction,
} from "../actions";
import { pathsFor } from "@/lib/paths";

// Fixed title: the destination name and URL never reach the document title.
export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).exports.integrations.title };
}

export default async function IntegrationDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, id } = await params;
  const text = (await getText()).exports;
  const t = text.integrations;
  const role = await currentRole();
  if (!can(role, "integration.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const cursor = parseCursor(await searchParams);
  const api = await authedApi();
  const integration = await loadOrNotFound(() => api.getIntegration(id));
  const deliveries = await loadOrNotFound(() => api.listIntegrationDeliveries(id, { cursor, limit: 50 }));
  const canManage = can(role, "integration.manage");
  const paths = pathsFor(org);
  const base = paths.admin.integration(id);
  return (
    <>
      <p><Link href={paths.admin.integrations}>{t.back}</Link></p>
      <h1>{integration.name}</h1>
      <p className="muted">
        {t.kinds[integration.kind]} | {t.environments[integration.environment]} | {t.statuses[integration.status]} |{" "}
        <TestBadge i={integration} />
      </p>
      {canManage ? (
        <>
          <h2>{t.form.editTitle}</h2>
          <IntegrationForm
            save={updateIntegrationAction.bind(null, org, id, integration.kind)}
            orgId={org}
            existing={integration}
          />
        </>
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
      {integration.kind === "webhook" ? (
        <SecretPanel integration={integration} put={putSecretAction.bind(null, org, id)} canManage={canManage} />
      ) : null}
      {integration.kind === "sftp" ? (
        <SshKeyPanel integration={integration} generate={generateSshKeyAction.bind(null, org, id)} canManage={canManage} />
      ) : null}
      <TestPanel test={testIntegrationAction.bind(null, org, id)} canManage={canManage} />
      {canManage ? (
        <DeleteIntegrationPanel
          name={integration.name}
          remove={deleteIntegrationAction.bind(null, org, id)}
          listHref={paths.admin.integrations}
        />
      ) : null}
      <h2>{t.deliveries.title}</h2>
      <DeliveriesTable
        items={deliveries.items}
        orgPath={paths.root}
        linkExports={can(role, "export.read")}
      />
      <p className="muted">{text.deliveries.showing(deliveries.items.length, deliveries.total)}</p>
      <nav className="pagination" aria-label={text.exports.pagination}>
        {cursor ? <Link href={base}>{text.deliveries.first}</Link> : null}
        {deliveries.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: deliveries.next_cursor })}`}>{text.deliveries.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
