import { catalogMessages } from "@/messages/catalog";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [
  "batteries.newIdExample", // an example clip id, not prose
];

describeMessages("catalog", catalogMessages, { NE_PENDING, NE_SAME_AS_EN });
