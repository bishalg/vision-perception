import { connection } from "next/server";
import { getText } from "@/lib/i18n/server";

/** Unmatched URLs. Rendered per request (CSP-1 nonce), like every page. */
export default async function NotFound() {
  await connection();
  const t = await getText();
  return (
    <main className="panel">
      <h1>{t.app.errors.notFound}</h1>
    </main>
  );
}
