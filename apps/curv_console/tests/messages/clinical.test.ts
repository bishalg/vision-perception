import { clinicalMessages } from "@/messages/clinical";
import { describeMessages } from "./helpers";

/** WP-L.1b: `ne` is a machine draft (`// ne: draft`), so its content is asserted. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [
  "review.limit", // "n / max"
  "review.signedBy", // a name and a registration number
  "reloadSlugs.0", // problem slugs, not text
  "reloadSlugs.1",
  "settings.timezonePlaceholder", // an IANA zone example
  "settings.papers.a4", // paper size
];

describeMessages("clinical", clinicalMessages, { NE_PENDING, NE_SAME_AS_EN });
