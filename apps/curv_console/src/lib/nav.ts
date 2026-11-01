/** Main navigation entries for a role. Hides links only; the backend authorises every request. */
import type { Role } from "@/lib/api";
import { can } from "@/lib/permissions";
import { adminText } from "@/messages/admin";
import { en } from "@/messages/en";

export function navLinks(base: string, role: Role | undefined): { href: string; text: string }[] {
  return [
    { href: `${base}/overview`, text: en.nav.overview },
    { href: `${base}/sessions`, text: en.nav.sessions },
    { href: `${base}/participants`, text: adminText.nav.participants },
    { href: `${base}/review`, text: en.nav.review },
    ...(can(role, "worklist.read") ? [{ href: `${base}/worklist`, text: en.nav.worklist }] : []),
    ...(can(role, "device.read") ? [{ href: `${base}/devices`, text: en.nav.devices }] : []),
    ...(can(role, "export.read") ? [{ href: `${base}/exports`, text: en.nav.exports }] : []),
    ...(can(role, "user.read") ? [{ href: `${base}/users`, text: adminText.nav.users }] : []),
    ...(can(role, "audit.read") ? [{ href: `${base}/audit`, text: adminText.nav.audit }] : []),
    ...(can(role, "worklist.read") || can(role, "org.update")
      ? [{ href: `${base}/settings/clinical`, text: en.nav.clinicalSettings }]
      : []),
    ...(can(role, "integration.read") ? [{ href: `${base}/settings/integrations`, text: en.nav.integrations }] : []),
  ];
}
