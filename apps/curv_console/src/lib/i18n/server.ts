import "server-only";
import { cache } from "react";
import { cookies, headers } from "next/headers";
import { ACCESS_COOKIE } from "@/lib/auth/cookies";
import { getApi, type UserProfile } from "@/lib/api";
import { messagesFor } from "./messages";
import { matchAcceptLanguage } from "./negotiate";
import { DEFAULT_LOCALE, isLocale, type Locale } from "./types";

/**
 * `GET /users/me` once per request (React `cache`), or null without a session or when the call fails. Pages that need
 * the profile can share this read instead of calling `api.getMyProfile()` again.
 */
export const myProfile = cache(async (): Promise<UserProfile | null> => {
  const token = (await cookies()).get(ACCESS_COOKIE)?.value;
  if (!token) return null;
  try {
    return await getApi(token).getMyProfile();
  } catch {
    return null;
  }
});

/**
 * I18N-16: with a session the backend's `effective_locale` (user > org > Accept-Language > en); without one (login,
 * `/share/*`, not-found) the `Accept-Language` match, else `en`. An older backend that omits the field yields `en`.
 */
export const resolveLocale = cache(async (): Promise<Locale> => {
  const profile = await myProfile();
  if (profile) return isLocale(profile.effective_locale) ? profile.effective_locale : DEFAULT_LOCALE;
  return matchAcceptLanguage((await headers()).get("accept-language")) ?? DEFAULT_LOCALE;
});

/** The whole message bundle for this request's locale. */
export const getText = cache(async () => messagesFor(await resolveLocale()));
