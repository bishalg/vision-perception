import { exportsMessages } from "@/messages/exports";
import { describeMessages } from "./helpers";

/** True while `ne` is the `// ne: TODO` copy of `en`; the owning wave package sets it to `false`. */
const NE_PENDING = false;
/** Paths whose `ne` text may equal `en` (brand names, units, codes such as MRN, TOTP, CSV). */
const NE_SAME_AS_EN: readonly string[] = [
  "exports.formats.csv", // file-format names
  "exports.formats.fhir_ndjson",
  "exports.formats.dataset_jsonl",
  "exports.formats.dataset_parquet",
  "exports.formats.audit_csv",
  "exports.detail.sha256", // hash algorithm name
  "integrations.kinds.sftp", // protocol names
  "integrations.kinds.hl7_mllp",
  "integrations.form.urlPlaceholder", // literal input examples, not prose
  "integrations.form.portPlaceholder",
  "integrations.form.remoteDirPlaceholder",
  "analysis.encodings.jsonl",
  "analysis.encodings.csv",
  "analysis.encodings.parquet",
];

describeMessages("exports", exportsMessages, { NE_PENDING, NE_SAME_AS_EN });
