import { configMessages } from "@/messages/config";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [];

describeMessages("config", configMessages, { NE_PENDING, NE_SAME_AS_EN });
