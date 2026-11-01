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
import { deliveriesText, integrationsText } from "@/messages/exports";
import {
  deleteIntegrationAction,
  generateSshKeyAction,
  putSecretAction,
  testIntegrationAction,
  updateIntegrationAction,
} from "../actions";

const t = integrationsText;

// Fixed title: the destination name and URL never reach the document title.
export const metadata: Metadata = { title: t.title };

export default async function IntegrationDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org, id } = await params;
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
  const base = `/${encodeURIComponent(org)}/settings/integrations/${encodeURIComponent(id)}`;
  return (
    <>
      <p><Link href={`/${encodeURIComponent(org)}/settings/integrations`}>{t.back}</Link></p>
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
          listHref={`/${encodeURIComponent(org)}/settings/integrations`}
        />
      ) : null}
      <h2>{t.deliveries.title}</h2>
      <DeliveriesTable
        items={deliveries.items}
        exportHref={can(role, "export.read") ? (exportId) => `/${encodeURIComponent(org)}/exports/${encodeURIComponent(exportId)}` : undefined}
      />
      <p className="muted">{deliveriesText.showing(deliveries.items.length, deliveries.total)}</p>
      <nav className="pagination" aria-label="Pagination">
        {cursor ? <Link href={base}>{deliveriesText.first}</Link> : null}
        {deliveries.next_cursor ? (
          <Link rel="next" href={`${base}${toSearch({ cursor: deliveries.next_cursor })}`}>{deliveriesText.next}</Link>
        ) : null}
      </nav>
    </>
  );
}
