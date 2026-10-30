import { redirect } from "next/navigation";
import { homeOrg } from "@/lib/auth/server";

export default async function Home() {
  const org = await homeOrg();
  redirect(org ? `/${encodeURIComponent(org)}/overview` : "/login");
}
