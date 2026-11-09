/**
 * WP-C.23 (spec 03 §3a, N-1/N-5): the console's navigation tree. Five divisions in the top bar, each with the
 * sidebar items a role may use. Hides links only; the backend authorises every request.
 */
import type { Role } from "@/lib/api";
import { messagesFor, type Messages } from "@/lib/i18n/messages";
import { orgPaths } from "@/lib/paths";
import { can } from "@/lib/permissions";

export type DivisionKey = "clinic" | "research" | "config" | "data" | "admin";

export interface NavItem {
  href: string;
  text: string;
}

export interface Division {
  key: DivisionKey;
  /** Where the top-bar link goes: the division's landing page (research: its index; others: the first item). */
  href: string;
  /** Path prefix that makes the division active (N-2). */
  prefix: string;
  text: string;
  items: NavItem[];
}

/**
 * The visible tree for `role` (N-1): a division appears only when at least one of its items is visible.
 * `base` is `/{org}` (see `orgBase`); `t` is the request's message bundle (unit tests get English).
 */
export function navTree(base: string, role: Role | undefined, t: Messages = messagesFor("en")): Division[] {
  const p = orgPaths(base);
  const when = (ok: boolean, item: NavItem): NavItem[] => (ok ? [item] : []);
  const divisions: Division[] = [
    {
      key: "clinic",
      href: p.clinic.overview,
      prefix: p.clinic.root,
      text: t.app.nav.divisions.clinic,
      items: [
        { href: p.clinic.overview, text: t.app.nav.overview },
        ...when(can(role, "worklist.read"), { href: p.clinic.worklist, text: t.app.nav.worklist }),
        { href: p.clinic.participants, text: t.admin.nav.participants },
        // WP-C.18: open follow-ups for the staff who record visits (REC-4).
        ...when(can(role, "encounter.write"), { href: p.clinic.followUps, text: t.admin.nav.followUps }),
        { href: p.clinic.review, text: t.app.nav.review },
        { href: p.clinic.sessions, text: t.app.nav.sessions },
      ],
    },
    {
      // Phase 8 RUI-1: the research area for everyone who reads analysis results.
      key: "research",
      href: p.research.root,
      prefix: p.research.root,
      text: t.app.nav.divisions.research,
      items: can(role, "analysis.read")
        ? [
            { href: p.research.experiments, text: t.research.experiments.title },
            { href: p.research.cohorts, text: t.research.cohorts.title },
            { href: p.research.runs, text: t.research.runs.title },
          ]
        : [],
    },
    {
      key: "config",
      href: p.config.catalog,
      prefix: p.config.root,
      text: t.app.nav.divisions.config,
      items: can(role, "config.read")
        ? [
            { href: p.config.catalog, text: t.catalog.nav },
            { href: p.config.baselines, text: t.config.nav.baselines },
            { href: p.config.scoring, text: t.config.nav.scoring },
            { href: p.config.sandbox, text: t.config.nav.sandbox },
            { href: p.config.bundles, text: t.config.nav.bundles },
          ]
        : [],
    },
    {
      key: "data",
      href: "",
      prefix: p.data.root,
      text: t.app.nav.divisions.data,
      items: [
        ...when(can(role, "export.read"), { href: p.data.exports, text: t.app.nav.exports }),
        ...when(can(role, "participant.erase"), { href: p.data.erasure, text: t.privacy.nav.erasure }),
        ...when(can(role, "participant.erase"), { href: p.data.retention, text: t.privacy.nav.retention }),
        ...when(can(role, "audit.read"), { href: p.data.audit, text: t.app.audit.title }),
      ],
    },
    {
      key: "admin",
      href: "",
      prefix: p.admin.root,
      text: t.app.nav.divisions.admin,
      items: [
        ...when(can(role, "user.read"), { href: p.admin.users, text: t.admin.nav.users }),
        ...when(can(role, "device.read"), { href: p.admin.devices, text: t.app.nav.devices }),
        ...when(can(role, "integration.read"), { href: p.admin.integrations, text: t.app.nav.integrations }),
        ...when(can(role, "worklist.read") || can(role, "org.update"), {
          href: p.admin.clinical,
          text: t.app.nav.clinicalSettings,
        }),
      ],
    },
  ];
  return divisions
    .filter((d) => d.items.length > 0)
    .map((d) => (d.href ? d : { ...d, href: d.items[0]!.href }));
}

/** True when `pathname` is `prefix` or below it (`/o/clinic` matches `/o/clinic/sessions/1`, not `/o/clinical`). */
export function underPath(pathname: string, prefix: string): boolean {
  return pathname === prefix || pathname.startsWith(`${prefix}/`);
}

/** N-2: the division whose prefix holds `pathname`, and the sidebar item whose path holds it (detail pages included). */
export function activeNav(
  tree: readonly Division[],
  pathname: string,
): { division: Division | undefined; item: NavItem | undefined } {
  const division = tree.find((d) => underPath(pathname, d.prefix));
  // Longest match wins, so a nested item path never loses to its parent.
  const item = division?.items
    .filter((i) => underPath(pathname, i.href))
    .sort((a, b) => b.href.length - a.href.length)[0];
  return { division, item };
}

/** Every sidebar item visible for `role`, across divisions (in tree order). */
export function navItems(base: string, role: Role | undefined, t: Messages = messagesFor("en")): NavItem[] {
  return navTree(base, role, t).flatMap((d) => d.items);
}
