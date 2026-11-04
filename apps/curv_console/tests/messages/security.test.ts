import { securityMessages } from "@/messages/security";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [
  "security.language.english", // a language is named in its own script in every locale
  "security.language.nepali",
  "security.orgLanguage.english",
  "security.orgLanguage.nepali",
];

describeMessages("security", securityMessages, { NE_PENDING, NE_SAME_AS_EN });
