import { appMessages } from "@/messages/en";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal the `en` text: the format is the same in both languages, or it is a bare symbol. */
const NE_SAME_AS_EN: readonly string[] = [
  "review.done", // `${action}: ${who}`: the pattern is the same; the action label is already Nepali
  "detail.tracking.drift.limit", // `${label} ${deg}°`: the pattern is the same
  "detail.tracking.drift.missedMark", // `× ${missed}`: the pattern is the same
  "audit.noChange", // "-": a dash, nothing to translate
];

describeMessages("app", appMessages, { NE_PENDING, NE_SAME_AS_EN });
