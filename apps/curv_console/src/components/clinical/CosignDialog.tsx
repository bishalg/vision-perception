"use client";

import { StepUpDialog, type StepUpDialogProps } from "./SignDialog";

/** Second signature (SIGN-6): same step-up, same displayed hash; the backend refuses the first signer. */
export function CosignDialog(props: Omit<StepUpDialogProps, "kind">) {
  return <StepUpDialog kind="cosign" {...props} />;
}
