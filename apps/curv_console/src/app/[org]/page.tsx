import { redirect } from "next/navigation";
import { pathsFor } from "@/lib/paths";

/** WP-C.23: `/{org}` opens the Clinic division's first page. */
export default async function OrgHome({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  redirect(pathsFor(org).clinic.overview);
}
