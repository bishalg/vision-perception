"use client";

/** Opens the browser print dialog; the print layout comes from `@media print` in participant.css. */
export function PrintButton({ label }: { label: string }) {
  return (
    <button type="button" onClick={() => window.print()}>
      {label}
    </button>
  );
}
