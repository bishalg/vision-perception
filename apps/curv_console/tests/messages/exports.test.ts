import { exportsMessages } from "@/messages/exports";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = true;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [];

describeMessages("exports", exportsMessages, { NE_PENDING, NE_SAME_AS_EN });
