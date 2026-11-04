import { participantMessages } from "@/messages/participant";
import { describeMessages } from "./helpers";

/** WP-L.1b: `ne` is a machine draft (`// ne: draft`), so its content is asserted. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [
  "header.mrn", // code
  "tabs.mrn", // code
  "mrn.number", // code
  // Functions whose sample output is only the sample arguments joined by punctuation (same in both locales).
  "timeline.withStatus",
  "timeline.attachment",
  "visits.visitLine",
  "notes.byline",
  "disclosures.delivery",
];

describeMessages("participant", participantMessages, { NE_PENDING, NE_SAME_AS_EN });
