/** User-visible strings for the account security page (WP-C.15 TOTP enrolment). en only; merged by import. */
import type { Widen } from "@/lib/i18n/types";

const errors: Record<string, string> = {
  "invalid-credentials": "The password is not correct.",
  "totp-invalid": "That code is not correct or was already used. Wait for the next code and try again.",
  "totp-required": "Enter a code from your authenticator app.",
  "totp-already-enrolled": "Two-step sign-in is already on. Turn it off first to set up a new authenticator.",
  "totp-enrolment-required":
    "Your role requires two-step sign-in. Set it up on the account security page to continue.",
  "totp-enrolment-expired": "The setup expired. Start again to get a new key.",
  "totp-not-enrolled": "Two-step sign-in is not on for your account.",
  "clinical-crypto-unavailable": "The server cannot store authenticator keys right now. Contact your administrator.",
  "rate-limited": "Too many attempts. Wait a minute and try again.",
  "validation-failed": "Some fields were rejected.",
  unavailable: "Could not reach the server. Try again shortly.",
  fallback: "The request failed. Try again.",
};

/** `table` is `t.security.errors` of the active locale (default: English, for callers outside this package). */
export function securityErrorMessage(slug: string | null, status: number, table: Record<string, string> = errors): string {
  if (slug && table[slug]) return table[slug];
  if (status === 429) return table["rate-limited"]!;
  if (status === 0) return table.unavailable!;
  return table.fallback!;
}

export const securityText = {
  errors,
  nav: "Account security",
  nudge: {
    text: "Your role requires two-step sign-in. Turn on two-step sign-in to protect your account.",
    deadline: (date: string) => `Set it up by ${date}.`,
    link: "Set it up",
  },
  required: {
    title: "Your role requires two-step sign-in",
    intro: "The time to set it up has passed. Set up two-step sign-in below to continue; other pages stay locked until you do.",
  },
  status: {
    pending: "A setup was started and is waiting for a code. Get a new key below to start again.",
  },
  title: "Account security",
  intro:
    "Two-step sign-in asks for a code from an authenticator app after your password. Some roles must turn it on.",
  enrol: {
    title: "Set up two-step sign-in",
    intro: "Confirm your password to get a new key for your authenticator app.",
    password: "Current password",
    start: "Get a key",
    qr: "QR code for your authenticator app",
    scan: "Scan the QR code with your authenticator app, or enter the key by hand.",
    secret: "Key",
    once: "This key is shown only once. Do not share it or store it with your password.",
    expires: (when: string) => `Confirm before ${when}.`,
    code: "Code from your authenticator app",
    confirm: "Confirm",
    done: "Two-step sign-in is on. You will be asked for a code each time you sign in.",
  },
  disable: {
    title: "Turn off two-step sign-in",
    intro: "Confirm your password and a current code from your authenticator app.",
    password: "Current password",
    code: "Code from your authenticator app",
    submit: "Turn off",
    done: "Two-step sign-in is off.",
  },
  /** I18N-20: the user's own language (`PATCH /users/me` `locale`; "follow the clinic" is `null`). */
  language: {
    title: "Language",
    intro: "The language of this console for your account. It applies to every page after you save.",
    label: "Language",
    follow: (org: string) => `System default (organisation: ${org})`,
    followUnknown: "System default (organisation)",
    english: "English",
    nepali: "नेपाली",
    save: "Save language",
    saved: "Language saved.",
    failed: "The language was not saved.",
  },
  /** I18N-20: the organisation's default language (`PATCH /orgs/{id}` `default_locale`; owner and admin). */
  orgLanguage: {
    title: "Default language for this organisation",
    intro: "Users who have not chosen their own language see the console in this language.",
    label: "Default language",
    english: "English",
    nepali: "नेपाली",
    save: "Save default language",
    saved: "Default language saved.",
    failed: "The default language was not saved.",
  },
  pagination: "Pagination",
} as const;

/** HIP-2 automatic logoff banners on /login (`?reason=idle|expired`). */
export const logoffText = {
  idle: "You were signed out because the console was idle for 30 minutes. Sign in again to continue.",
  expired: "Your session reached its 12-hour limit. Sign in again to continue.",
} as const;

/** WP-L.1.0 (I18N-15): both locales of this file. `ne` is typed by the `en` tree; the wave package replaces the copy. */
const en = { security: securityText, logoff: logoffText };
// ne: draft (machine draft; WP-L.3 reviews it)
const ne: Widen<typeof en> = {
  security: {
    errors: {
      "invalid-credentials": "पासवर्ड मिलेन।",
      "totp-invalid": "त्यो कोड मिलेन वा पहिले नै प्रयोग भइसकेको छ। अर्को कोड आउने बेलासम्म पर्खनुहोस् र फेरि प्रयास गर्नुहोस्।",
      "totp-required": "आफ्नो प्रमाणीकरण एपको कोड हाल्नुहोस्।",
      "totp-already-enrolled": "दुई-चरण साइन इन पहिले नै खुला छ। नयाँ प्रमाणीकरण एप मिलाउन पहिले यसलाई बन्द गर्नुहोस्।",
      "totp-enrolment-required":
        "तपाईंको भूमिकाका लागि दुई-चरण साइन इन आवश्यक छ। अगाडि बढ्न खाता सुरक्षा पृष्ठमा गएर यसलाई मिलाउनुहोस्।",
      "totp-enrolment-expired": "मिलाउने समय सकियो। नयाँ कुञ्जी पाउन फेरि सुरु गर्नुहोस्।",
      "totp-not-enrolled": "तपाईंको खातामा दुई-चरण साइन इन खुला छैन।",
      "clinical-crypto-unavailable": "सर्भरले अहिले प्रमाणीकरण कुञ्जी सुरक्षित राख्न सक्दैन। आफ्नो प्रशासकलाई सम्पर्क गर्नुहोस्।",
      "rate-limited": "धेरै पटक प्रयास भयो। एक मिनेट पर्खनुहोस् र फेरि प्रयास गर्नुहोस्।",
      "validation-failed": "केही फिल्ड स्वीकार गरिएनन्।",
      unavailable: "सर्भरमा पुग्न सकिएन। केही बेरपछि फेरि प्रयास गर्नुहोस्।",
      fallback: "अनुरोध असफल भयो। फेरि प्रयास गर्नुहोस्।",
    },
    nav: "खाता सुरक्षा",
    nudge: {
      text: "तपाईंको भूमिकाका लागि दुई-चरण साइन इन आवश्यक छ। आफ्नो खाता सुरक्षित गर्न दुई-चरण साइन इन खोल्नुहोस्।",
      deadline: (date: string) => `${date} सम्ममा मिलाउनुहोस्।`,
      link: "मिलाउनुहोस्",
    },
    required: {
      title: "तपाईंको भूमिकाका लागि दुई-चरण साइन इन आवश्यक छ",
      intro: "मिलाउने समय सकिएको छ। अगाडि बढ्न तल दुई-चरण साइन इन मिलाउनुहोस्; यो नमिलाएसम्म अरू पृष्ठ बन्द रहन्छन्।",
    },
    status: {
      pending: "मिलाउने काम सुरु भएको छ र कोडको प्रतीक्षामा छ। फेरि सुरु गर्न तल नयाँ कुञ्जी लिनुहोस्।",
    },
    title: "खाता सुरक्षा",
    intro: "दुई-चरण साइन इनले पासवर्डपछि प्रमाणीकरण एपको कोड माग्छ। केही भूमिकाका लागि यसलाई खोल्नैपर्छ।",
    enrol: {
      title: "दुई-चरण साइन इन मिलाउनुहोस्",
      intro: "आफ्नो प्रमाणीकरण एपका लागि नयाँ कुञ्जी पाउन पासवर्ड पुष्टि गर्नुहोस्।",
      password: "हालको पासवर्ड",
      start: "कुञ्जी लिनुहोस्",
      qr: "प्रमाणीकरण एपका लागि QR कोड",
      scan: "प्रमाणीकरण एपले QR कोड स्क्यान गर्नुहोस्, वा कुञ्जी हातैले हाल्नुहोस्।",
      secret: "कुञ्जी",
      once: "यो कुञ्जी एक पटक मात्र देखाइन्छ। कसैलाई नदिनुहोस् र पासवर्डसँगै नराख्नुहोस्।",
      expires: (when: string) => `${when} अगावै पुष्टि गर्नुहोस्।`,
      code: "प्रमाणीकरण एपको कोड",
      confirm: "पुष्टि गर्नुहोस्",
      done: "दुई-चरण साइन इन खुला छ। अब हरेक पटक साइन इन गर्दा कोड सोधिन्छ।",
    },
    disable: {
      title: "दुई-चरण साइन इन बन्द गर्नुहोस्",
      intro: "पासवर्ड र प्रमाणीकरण एपको हालको कोड पुष्टि गर्नुहोस्।",
      password: "हालको पासवर्ड",
      code: "प्रमाणीकरण एपको कोड",
      submit: "बन्द गर्नुहोस्",
      done: "दुई-चरण साइन इन बन्द छ।",
    },
    language: {
      title: "भाषा",
      intro: "तपाईंको खातामा यो कन्सोलको भाषा। सुरक्षित गरेपछि सबै पृष्ठमा लागू हुन्छ।",
      label: "भाषा",
      follow: (org: string) => `प्रणालीको पूर्वनिर्धारित (संस्था: ${org})`,
      followUnknown: "प्रणालीको पूर्वनिर्धारित (संस्था)",
      english: "English",
      nepali: "नेपाली",
      save: "भाषा सुरक्षित गर्नुहोस्",
      saved: "भाषा सुरक्षित गरियो।",
      failed: "भाषा सुरक्षित गरिएन।",
    },
    orgLanguage: {
      title: "यो संस्थाको पूर्वनिर्धारित भाषा",
      intro: "आफ्नो भाषा नछानेका प्रयोगकर्ताले कन्सोल यही भाषामा देख्छन्।",
      label: "पूर्वनिर्धारित भाषा",
      english: "English",
      nepali: "नेपाली",
      save: "पूर्वनिर्धारित भाषा सुरक्षित गर्नुहोस्",
      saved: "पूर्वनिर्धारित भाषा सुरक्षित गरियो।",
      failed: "पूर्वनिर्धारित भाषा सुरक्षित गरिएन।",
    },
    pagination: "पृष्ठ सूची",
  },
  logoff: {
    idle: "कन्सोल ३० मिनेट निष्क्रिय रहेकाले तपाईं साइन आउट हुनुभयो। अगाडि बढ्न फेरि साइन इन गर्नुहोस्।",
    expired: "तपाईंको सत्रले १२ घण्टाको सीमा पुग्यो। अगाडि बढ्न फेरि साइन इन गर्नुहोस्।",
  },
};
export const securityMessages = { en, ne } as const;
