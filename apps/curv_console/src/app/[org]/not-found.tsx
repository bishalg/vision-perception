import { getText } from "@/lib/i18n/server";

export default async function NotFound() {
  const t = await getText();
  return <h1>{t.app.errors.notFound}</h1>;
}
