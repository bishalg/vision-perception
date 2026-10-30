"use client";

import { en } from "@/messages/en";

export default function OrgError({ reset }: { error: Error; reset: () => void }) {
  return (
    <div className="panel" role="alert">
      <h1>{en.errors.title}</h1>
      <button type="button" onClick={reset}>
        {en.errors.retry}
      </button>
    </div>
  );
}
