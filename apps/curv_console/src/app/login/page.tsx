import type { Metadata } from "next";
import { apiMode } from "@/lib/api";
import { safeNextPath } from "@/lib/auth/guard";
import { en } from "@/messages/en";
import { loginAction } from "./actions";
import { LoginForm } from "./LoginForm";

export const metadata: Metadata = { title: en.login.title };

export default async function LoginPage({ searchParams }: { searchParams: Promise<{ next?: string | string[] }> }) {
  const sp = await searchParams;
  const raw = Array.isArray(sp.next) ? sp.next[0] : sp.next;
  const next = safeNextPath(raw) ?? undefined;
  return (
    <main className="login-page">
      <div className="login-card">
        <p className="brand">{en.appName}</p>
        <LoginForm action={loginAction} next={next} mockHint={apiMode() === "mock"} />
      </div>
    </main>
  );
}
