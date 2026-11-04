import { adminMessages } from "@/messages/admin";
import { appMessages } from "@/messages/en";
import { catalogMessages } from "@/messages/catalog";
import { clinicalMessages } from "@/messages/clinical";
import { configMessages } from "@/messages/config";
import { DOMAIN, dt, enumLabel } from "@/messages/domain";
import { exportsMessages } from "@/messages/exports";
import { participantMessages } from "@/messages/participant";
import { privacyMessages } from "@/messages/privacy";
import { researchMessages } from "@/messages/research";
import { securityMessages } from "@/messages/security";
import type { Locale } from "./types";

/** Everything a component needs in one locale (I18N-16). */
export function messagesFor(locale: Locale) {
  return {
    locale,
    app: appMessages[locale],
    admin: adminMessages[locale],
    catalog: catalogMessages[locale],
    clinical: clinicalMessages[locale],
    config: configMessages[locale],
    exports: exportsMessages[locale],
    participant: participantMessages[locale],
    privacy: privacyMessages[locale],
    research: researchMessages[locale],
    security: securityMessages[locale],
    domain: { table: DOMAIN[locale], dt, enumLabel },
  };
}

export type Messages = ReturnType<typeof messagesFor>;
