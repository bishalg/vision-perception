/** Main navigation entries for a role. Hides links only; the backend authorises every request. */
import type { Role } from "@/lib/api";
import { messagesFor, type Messages } from "@/lib/i18n/messages";
import { can } from "@/lib/permissions";

/** `t` is the request's message bundle; callers without one (unit tests) get English. */
export function navLinks(
  base: string,
  role: Role | undefined,
  t: Messages = messagesFor("en"),
): { href: string; text: string }[] {
  return [
    { href: `${base}/overview`, text: t.app.nav.overview },
    { href: `${base}/sessions`, text: t.app.nav.sessions },
    { href: `${base}/participants`, text: t.admin.nav.participants },
    // WP-C.18: open follow-ups for the staff who record visits (REC-4).
    ...(can(role, "encounter.write") ? [{ href: `${base}/follow-ups`, text: t.admin.nav.followUps }] : []),
    { href: `${base}/review`, text: t.app.nav.review },
    ...(can(role, "worklist.read") ? [{ href: `${base}/worklist`, text: t.app.nav.worklist }] : []),
    ...(can(role, "device.read") ? [{ href: `${base}/devices`, text: t.app.nav.devices }] : []),
    ...(can(role, "export.read") ? [{ href: `${base}/exports`, text: t.app.nav.exports }] : []),
    ...(can(role, "user.read") ? [{ href: `${base}/users`, text: t.admin.nav.users }] : []),
    ...(can(role, "audit.read") ? [{ href: `${base}/audit`, text: t.app.audit.title }] : []),
    ...(can(role, "worklist.read") || can(role, "org.update")
      ? [{ href: `${base}/settings/clinical`, text: t.app.nav.clinicalSettings }]
      : []),
    ...(can(role, "integration.read") ? [{ href: `${base}/settings/integrations`, text: t.app.nav.integrations }] : []),
    ...(can(role, "participant.erase") ? [{ href: `${base}/erasure`, text: t.privacy.nav.erasure }] : []),
    ...(can(role, "participant.erase") ? [{ href: `${base}/settings/retention`, text: t.privacy.nav.retention }] : []),
    ...(can(role, "config.read") ? [{ href: `${base}/config/catalog`, text: t.catalog.nav }] : []),
    ...(can(role, "config.read")
      ? [
          { href: `${base}/config/baselines`, text: t.config.nav.baselines },
          { href: `${base}/config/scoring`, text: t.config.nav.scoring },
          { href: `${base}/config/sandbox`, text: t.config.nav.sandbox },
          { href: `${base}/config/bundles`, text: t.config.nav.bundles },
        ]
      : []),
    // Phase 8 RUI-1: the research area (pages from WP-X.5 / WP-X.7) for everyone who reads analysis results.
    ...(can(role, "analysis.read") ? [{ href: `${base}/research`, text: t.research.nav }] : []),
    // WP-C.15: every signed-in user manages their own second factor.
    { href: `${base}/settings/security`, text: t.security.security.nav },
  ];
}
