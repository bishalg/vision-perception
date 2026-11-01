/** Main navigation entries for a role. Hides links only; the backend authorises every request. */
import type { Role } from "@/lib/api";
import { can } from "@/lib/permissions";
import { adminText } from "@/messages/admin";
import { catalogText } from "@/messages/catalog";
import { configText } from "@/messages/config";
import { en } from "@/messages/en";
import { privacyText } from "@/messages/privacy";
import { securityText } from "@/messages/security";

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
    ...(can(role, "participant.erase") ? [{ href: `${base}/erasure`, text: privacyText.nav.erasure }] : []),
    ...(can(role, "participant.erase") ? [{ href: `${base}/settings/retention`, text: privacyText.nav.retention }] : []),
    ...(can(role, "config.read") ? [{ href: `${base}/config/catalog`, text: catalogText.nav }] : []),
    ...(can(role, "config.read")
      ? [
          { href: `${base}/config/baselines`, text: configText.nav.baselines },
          { href: `${base}/config/scoring`, text: configText.nav.scoring },
          { href: `${base}/config/sandbox`, text: configText.nav.sandbox },
          { href: `${base}/config/bundles`, text: configText.nav.bundles },
        ]
      : []),
    // WP-C.15: every signed-in user manages their own second factor.
    { href: `${base}/settings/security`, text: securityText.nav },
  ];
}
