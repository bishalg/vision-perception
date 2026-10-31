import "server-only";
import { notFound, redirect } from "next/navigation";
import { ApiError } from "@/lib/api";

/** Map API errors to Next navigation outcomes: 404 -> not-found, 401 -> login, anything else bubbles to error.tsx. */
export async function loadOrNotFound<T>(load: () => Promise<T>): Promise<T> {
  try {
    return await load();
  } catch (err) {
    if (err instanceof ApiError) {
      if (err.status === 404 || err.status === 403) notFound();
      if (err.status === 401) redirect("/login");
    }
    throw err;
  }
}

/**
 * Wall-clock time of this server render. Server components render once per request, so reading the clock is
 * safe here; it is passed to client components so relative times match between server and client.
 */
export function requestTime(): number {
  return Date.now();
}
