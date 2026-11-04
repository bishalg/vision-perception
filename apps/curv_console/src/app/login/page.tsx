import type { Metadata } from "next";
import { apiMode } from "@/lib/api";
import { safeNextPath } from "@/lib/auth/guard";
import { getText } from "@/lib/i18n/server";
import { loginAction } from "./actions";
import { LoginForm } from "./LoginForm";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).app.login.title };
}

export default async function LoginPage({ searchParams }: { searchParams: Promise<{ next?: string | string[]; reason?: string | string[] }> }) {
  const t = await getText();
  const sp = await searchParams;
  const raw = Array.isArray(sp.next) ? sp.next[0] : sp.next;
  const next = safeNextPath(raw) ?? undefined;
  const reasonRaw = Array.isArray(sp.reason) ? sp.reason[0] : sp.reason;
  const reason = reasonRaw === "idle" || reasonRaw === "expired" ? reasonRaw : undefined;
  return (
    <main className="login-page">
      <div className="login-card">
        <p className="brand">{t.app.appName}</p>
        {reason ? (
          <p role="status" className="muted" data-testid="logoff-reason">
            {t.security.logoff[reason]}
          </p>
        ) : null}
        <LoginForm action={loginAction} next={next} mockHint={apiMode() === "mock"} />
      </div>
    </main>
  );
}
