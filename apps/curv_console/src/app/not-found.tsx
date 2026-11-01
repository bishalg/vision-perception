import { connection } from "next/server";
import { en } from "@/messages/en";

/** Unmatched URLs. Rendered per request (CSP-1 nonce), like every page. */
export default async function NotFound() {
  await connection();
  return (
    <main className="panel">
      <h1>{en.errors.notFound}</h1>
    </main>
  );
}
