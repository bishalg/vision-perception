import { redirect } from "next/navigation";
import { authedApi, homeOrg, recentOrgs } from "@/lib/auth/server";
import { en } from "@/messages/en";

const t = en.orgs;

/**
 * Home: users with an org go to its overview. The superadmin has no home org, so this page lets them open
 * one by id or pick a recent one (the backend has no org list endpoint yet).
 */
export default async function Home({ searchParams }: { searchParams: Promise<{ invalid?: string }> }) {
  const org = await homeOrg();
  if (org) redirect(`/${encodeURIComponent(org)}/overview`);
  const api = await authedApi();
  const recent = await Promise.allSettled((await recentOrgs()).map((id) => api.getOrg(id)));
  const orgs = recent.flatMap((r) => (r.status === "fulfilled" ? [r.value] : []));
  const invalid = (await searchParams).invalid === "1";
  return (
    <main className="login-page">
      <div className="login-card">
        <p className="brand">{en.appName}</p>
        <h1>{t.pickTitle}</h1>
        <p className="muted">{t.pickIntro}</p>
        <form method="get" action="/switch-org" className="login-form" aria-label={t.pickTitle}>
          <label>
            {t.byId}
            <input name="org_id" required autoComplete="off" />
          </label>
          {invalid ? <p role="alert" className="error">{t.invalid}</p> : null}
          <button type="submit">{t.switch}</button>
        </form>
        {orgs.length ? (
          <section aria-label={t.recent}>
            <h2>{t.recent}</h2>
            <ul>
              {orgs.map((o) => (
                <li key={o.id}>
                  <a href={`/${encodeURIComponent(o.id)}/overview`}>{o.name}</a>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>
    </main>
  );
}
