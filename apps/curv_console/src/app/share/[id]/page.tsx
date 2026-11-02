import type { Metadata } from "next";
import { sharePageText } from "@/messages/exports";
import { openShareAction } from "./actions";
import { ShareOpenForm } from "./ShareOpenForm";

const t = sharePageText;

// HIP-9: never indexed (next.config.ts also sends `X-Robots-Tag` and `no-store`). Nothing about the case in the title.
export const metadata: Metadata = { title: t.title, robots: { index: false, follow: false, nocache: true } };

/**
 * Public share page (SHR-9, SHR-10, WP-C.19): outside `[org]`, no session read or required, the same nonce CSP as
 * every page (proxy.ts). Rendering asks the backend nothing, so the page alone does not tell whether a link id
 * exists; only the passcode step reports the backend's answer (`404` vs `410`).
 */
export default async function SharePage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { id } = await params;
  const sp = await searchParams;
  const raw = Array.isArray(sp.e) ? sp.e[0] : sp.e;
  // Only known keys are shown (`?e=` comes back from the download route); anything else is ignored.
  const error = raw && Object.hasOwn(t.errors, raw) ? raw : null;
  return (
    <main className="login-page">
      <div className="login-card stack">
        <h1>{t.heading}</h1>
        <p>{t.intro}</p>
        <ShareOpenForm linkId={id} open={openShareAction.bind(null, id)} initialError={error} />
        <p className="muted">{t.notice}</p>
      </div>
    </main>
  );
}
