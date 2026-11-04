/**
 * WP-L.1c (I18N-20, I18N-22): the language selectors, their server actions, the org selector's visibility, the public
 * share page's Accept-Language negotiation and the exports/privacy/security components in Nepali.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ErasureTable } from "@/components/privacy/ErasureTable";
import { ExportsTable } from "@/components/exports/ExportsTable";
import { LanguageForm, OrgLanguageForm } from "@/components/security/LanguageForms";
import { TotpNudge } from "@/components/security/TotpNudge";
import type { Export } from "@/lib/api/exports";
import type { UserProfile } from "@/lib/api/clinical";
import { LocaleProvider } from "@/lib/i18n/provider";

vi.mock("server-only", () => ({}));
const revalidatePath = vi.fn();
vi.mock("next/cache", () => ({ revalidatePath: (...a: unknown[]) => revalidatePath(...a) }));
const updateMyProfile = vi.fn();
const updateOrgClinicalSettings = vi.fn();
const getOrg = vi.fn();
const getMyProfile = vi.fn();
let role: string | undefined = "admin";
vi.mock("@/lib/auth/server", () => ({
  authedApi: async () => ({ updateMyProfile, updateOrgClinicalSettings, getOrg, getMyProfile }),
  currentRole: async () => role,
}));
let acceptLanguage: string | null = null;
vi.mock("next/headers", () => ({
  headers: async () => new Headers(acceptLanguage ? { "accept-language": acceptLanguage } : {}),
  cookies: async () => ({ get: () => undefined }),
}));
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), refresh: vi.fn() }), redirect: vi.fn() }));
vi.mock("@/lib/i18n/server", async () => {
  const { messagesFor } = await import("@/lib/i18n/messages");
  const { matchAcceptLanguage } = await import("@/lib/i18n/negotiate");
  const locale = async () => matchAcceptLanguage(acceptLanguage) ?? "en";
  return { getText: async () => messagesFor(await locale()), resolveLocale: locale };
});

const DEVANAGARI = /[ऀ-ॿ]/;

beforeEach(() => {
  vi.clearAllMocks();
  role = "admin";
  acceptLanguage = null;
  getOrg.mockResolvedValue({ id: "o", name: "Demo", kind: "clinic", default_locale: "ne" });
  getMyProfile.mockResolvedValue({ id: "u", email: "a@b.c", role: "admin", locale: null, effective_locale: "en" });
  updateMyProfile.mockResolvedValue({});
  updateOrgClinicalSettings.mockResolvedValue({});
});

describe("user language selector", () => {
  it("offers three options and shows the current value", () => {
    render(<LanguageForm save={vi.fn()} current="ne" orgLocale="en" />);
    const select = screen.getByRole("combobox", { name: "Language" });
    expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "System default (organisation: English)",
      "English",
      "नेपाली",
    ]);
    expect(select).toHaveValue("ne");
  });

  it("follows the organisation when its own value is null, and names the organisation's language", () => {
    render(<LanguageForm save={vi.fn()} current={null} orgLocale="ne" />);
    expect(screen.getByRole("combobox")).toHaveValue("");
    expect(screen.getByRole("option", { name: "System default (organisation: नेपाली)" })).toBeInTheDocument();
  });

  it("submits the choice and reports the result in the chosen language", async () => {
    const save = vi.fn().mockResolvedValue({ ok: true });
    const { rerender } = render(
      <LocaleProvider locale="en">
        <LanguageForm save={save} current={null} orgLocale="en" />
      </LocaleProvider>,
    );
    await userEvent.selectOptions(screen.getByRole("combobox"), "ne");
    await userEvent.click(screen.getByRole("button", { name: "Save language" }));
    expect((save.mock.calls[0]![0] as FormData).get("locale")).toBe("ne");
    expect(await screen.findByRole("status")).toHaveTextContent("Language saved.");
    // The layout is revalidated by the action; the provider then hands the form the Nepali bundle.
    rerender(
      <LocaleProvider locale="ne">
        <LanguageForm save={save} current="ne" orgLocale="en" />
      </LocaleProvider>,
    );
    expect(screen.getByRole("status")).toHaveTextContent("भाषा सुरक्षित गरियो।");
    expect(screen.getByRole("button", { name: "भाषा सुरक्षित गर्नुहोस्" })).toBeInTheDocument();
  });

  it("shows a failure sentence", async () => {
    const save = vi.fn().mockResolvedValue({ ok: false, slug: "rate-limited", status: 429 });
    render(<LanguageForm save={save} current={null} orgLocale="en" />);
    await userEvent.click(screen.getByRole("button", { name: "Save language" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("The language was not saved. Too many attempts.");
  });
});

describe("saveLanguageAction", () => {
  const fd = (v: string) => {
    const f = new FormData();
    f.set("locale", v);
    return f;
  };

  it("calls updateMyProfile({ locale }) and revalidates the layout", async () => {
    const { saveLanguageAction } = await import("@/app/[org]/settings/security/actions");
    expect(await saveLanguageAction(fd("ne"))).toEqual({ ok: true });
    expect(updateMyProfile).toHaveBeenCalledWith({ locale: "ne" });
    expect(revalidatePath).toHaveBeenCalledWith("/", "layout");
  });

  it("sends null for the organisation default", async () => {
    const { saveLanguageAction } = await import("@/app/[org]/settings/security/actions");
    expect(await saveLanguageAction(fd(""))).toEqual({ ok: true });
    expect(updateMyProfile).toHaveBeenCalledWith({ locale: null });
  });

  it("refuses an unknown locale without calling the backend", async () => {
    const { saveLanguageAction } = await import("@/app/[org]/settings/security/actions");
    expect(await saveLanguageAction(fd("fr"))).toMatchObject({ ok: false, status: 400 });
    expect(updateMyProfile).not.toHaveBeenCalled();
  });
});

describe("org default language", () => {
  it("saveOrgLanguageAction patches default_locale and revalidates", async () => {
    const { saveOrgLanguageAction } = await import("@/app/[org]/settings/clinical/actions");
    const f = new FormData();
    f.set("default_locale", "ne");
    expect(await saveOrgLanguageAction("o1", f)).toEqual({ ok: true });
    expect(updateOrgClinicalSettings).toHaveBeenCalledWith("o1", {}, { default_locale: "ne" });
    expect(revalidatePath).toHaveBeenCalledWith("/", "layout");
  });

  it("OrgLanguageForm shows the current value", () => {
    render(<OrgLanguageForm save={vi.fn()} current="ne" />);
    expect(screen.getByRole("combobox", { name: "Default language" })).toHaveValue("ne");
  });

  it("the settings page shows the org selector to admin and hides it for clinician", async () => {
    const { default: Page } = await import("@/app/[org]/settings/clinical/page");
    getMyProfile.mockResolvedValue({ id: "u", email: "a@b.c", role: "admin", display_name: "A", professional_registration: "R" });
    render(await Page({ params: Promise.resolve({ org: "o" }) }));
    expect(screen.getByRole("region", { name: "Default language for this organisation" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Default language" })).toHaveValue("ne");
    expect(getOrg).toHaveBeenCalledWith("o");
    cleanupRender();

    role = "clinician";
    getOrg.mockClear();
    render(await Page({ params: Promise.resolve({ org: "o" }) }));
    expect(screen.queryByText("Default language for this organisation")).toBeNull();
    expect(getOrg).not.toHaveBeenCalled();
  });

  it("the security page shows the user selector to every role with the org's language in the first option", async () => {
    const { default: Page } = await import("@/app/[org]/settings/security/page");
    role = "clinician";
    getMyProfile.mockResolvedValue({ id: "u", email: "a@b.c", role: "clinician", locale: "en", effective_locale: "en", totp_enrolled: false });
    render(await Page({ params: Promise.resolve({ org: "o" }), searchParams: Promise.resolve({}) }));
    const select = screen.getByRole("combobox", { name: "Language" });
    expect(select).toHaveValue("en");
    expect(screen.getByRole("option", { name: "System default (organisation: नेपाली)" })).toBeInTheDocument();
  });
});

function cleanupRender() {
  document.body.innerHTML = "";
}

describe("public share page (I18N-22)", () => {
  const params = { params: Promise.resolve({ id: "l1" }), searchParams: Promise.resolve({}) };

  it("with Accept-Language: ne renders Nepali text plus the English mandatory sentence", async () => {
    acceptLanguage = "ne";
    const { default: SharePage } = await import("@/app/share/[id]/page");
    render(await SharePage(params));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toMatch(DEVANAGARI);
    expect(screen.getByText(/This is not a diagnostic tool\./)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "जारी राख्नुहोस्" })).toBeInTheDocument();
  });

  it("with no Accept-Language stays English and does not repeat the sentence", async () => {
    const { default: SharePage } = await import("@/app/share/[id]/page");
    render(await SharePage(params));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Download a shared file");
    expect(screen.queryByText(/This is not a diagnostic tool\./)).toBeNull();
  });

  it("ignores a signed-in user's setting: only Accept-Language counts", async () => {
    acceptLanguage = "fr";
    const { default: SharePage } = await import("@/app/share/[id]/page");
    render(await SharePage(params));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Download a shared file");
  });
});

describe("components in ne", () => {
  const exp: Export = {
    id: "4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01", org_id: "o", kind: "bulk", format: "csv", mode: "deidentified",
    purpose: "research", status: "succeeded", filters: {}, reason: "Quarterly research extract", destination_ids: [],
    created_at: "2026-10-31T09:00:00.000Z", excluded: { no_research_consent: 3, withdrawn: 1 }, row_count: 42,
  };

  it("ExportsTable headings, enums and excluded reasons are Nepali", () => {
    render(
      <LocaleProvider locale="ne">
        <ExportsTable orgId="o" items={[exp]} canDownload />
      </LocaleProvider>,
    );
    const row = screen.getAllByRole("row")[1]!;
    expect(within(row).getByText("तयार")).toHaveClass("badge-green");
    expect(within(row).getByText("पहिचान हटाइएको")).toBeInTheDocument();
    expect(within(row).getByText(/अनुसन्धान सहमति छैन 3/)).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "ढाँचा" })).toBeInTheDocument();
    expect(screen.queryByText("Ready")).toBeNull();
  });

  it("ErasureTable counts and status are Nepali", () => {
    render(
      <LocaleProvider locale="ne">
        <ErasureTable
          items={[{
            id: "r", org_id: "o", participant_id: "p", participant_token: "P-1", status: "held", requested_by_type: "user",
            requested_at: "2026-10-31T09:00:00Z", completed_at: null, hold_reason: "clinical_record",
            outcome: { sessions_deleted: 2, sessions_held: 1 },
          } as never]}
        />
      </LocaleProvider>,
    );
    expect(screen.getByText("रोकिएको")).toBeInTheDocument();
    expect(screen.getByText("2 सत्र मेटिए, 1 सत्र रोकिए")).toBeInTheDocument();
  });

  it("TotpNudge is Nepali", () => {
    const profile = { totp_enforcement: "grace", totp_enrolled: false, totp_required_by: null, role: "admin" } as unknown as UserProfile;
    render(
      <LocaleProvider locale="ne">
        <TotpNudge org="o" profile={profile} />
      </LocaleProvider>,
    );
    expect(screen.getByTestId("totp-nudge").textContent).toMatch(DEVANAGARI);
  });
});
