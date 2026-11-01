import type { Metadata } from "next";
import { IntegrationForm } from "@/components/exports/IntegrationForm";
import { IntegrationsTable } from "@/components/exports/IntegrationsTable";
import { authedApi, currentRole } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { integrationsText } from "@/messages/exports";
import { createIntegrationAction } from "./actions";

const t = integrationsText;

export const metadata: Metadata = { title: t.title };

export default async function IntegrationsPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const role = await currentRole();
  if (!can(role, "integration.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{t.denied}</p>
      </>
    );
  }
  const api = await authedApi();
  const page = await loadOrNotFound(() => api.listIntegrations(org));
  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <IntegrationsTable orgId={org} items={page.items} />
      <h2>{t.form.createTitle}</h2>
      {can(role, "integration.manage") ? (
        <IntegrationForm save={createIntegrationAction.bind(null, org)} orgId={org} />
      ) : (
        <p className="muted">{t.readOnly}</p>
      )}
    </>
  );
}
