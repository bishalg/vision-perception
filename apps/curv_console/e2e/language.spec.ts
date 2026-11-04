/**
 * WP-L.1c (I18N-20; acceptance 8, mock mode): the user's own language on the security page and the organisation's
 * default language on the clinical settings page. Both re-render in the chosen language after saving.
 *
 * The mock keeps the user's choice per e-mail and the org default in memory, so each test uses its own e-mail
 * (a `+tag` that is not a locale does not seed a language) and the org default is put back at the end.
 */
import { expect, test, LIVE, MOCK_ORG_ID, login } from "./helpers";

test.skip(LIVE, "the mock backend's in-memory user and organisation locale");

const SECURITY = `/${MOCK_ORG_ID}/settings/security`;
const CLINICAL = `/${MOCK_ORG_ID}/settings/clinical`;

test("a user picks their own language on the security page and goes back to the organisation default", async ({ page }) => {
  await login(page, `clinician+lang1x${Date.now()}@example.org`);
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await page.goto(SECURITY);
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  const panel = page.getByRole("region", { name: "Language", exact: true });
  const select = panel.getByRole("combobox", { name: "Language" });
  await expect(select.locator("option")).toHaveText(["System default (organisation: English)", "English", "नेपाली"]);
  await expect(select).toHaveValue("");

  await select.selectOption("ne");
  await panel.getByRole("button", { name: "Save language" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
  await expect(page.getByRole("heading", { name: "खाता सुरक्षा", level: 1 })).toBeVisible();
  const nepali = page.getByRole("region", { name: "भाषा", exact: true });
  await expect(nepali.getByRole("status")).toHaveText("भाषा सुरक्षित गरियो।");
  await expect(nepali.getByRole("combobox")).toHaveValue("ne");

  // "System default" is null: back to the organisation's language (English here).
  await nepali.getByRole("combobox").selectOption("");
  await nepali.getByRole("button", { name: "भाषा सुरक्षित गर्नुहोस्" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByRole("heading", { name: "Account security", level: 1 })).toBeVisible();
});

test("an admin sets the organisation's default language; a clinician follows it and has no org selector", async ({ page, browser }) => {
  await login(page, "admin+lang2@example.org");
  await expect(page).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
  await page.goto(CLINICAL);
  const org = page.getByRole("region", { name: "Default language for this organisation" });
  await expect(org.getByRole("combobox", { name: "Default language" })).toHaveValue("en");
  try {
    await org.getByRole("combobox").selectOption("ne");
    await org.getByRole("button", { name: "Save default language" }).click();
    // The admin has no own setting, so the page re-renders in the new default.
    await expect(page.locator("html")).toHaveAttribute("lang", "ne-NP");
    await expect(page.getByRole("region", { name: "यो संस्थाको पूर्वनिर्धारित भाषा" }).getByRole("status")).toHaveText(
      "पूर्वनिर्धारित भाषा सुरक्षित गरियो।",
    );

    // A clinician without an own setting sees Nepali; the org selector is not offered to the role.
    const other = await browser.newContext();
    const clinician = await other.newPage();
    await login(clinician, "clinician+lang3@example.org");
    await expect(clinician).toHaveURL(/\/[0-9a-f-]{36}\/overview$/);
    await clinician.goto(CLINICAL);
    await expect(clinician.locator("html")).toHaveAttribute("lang", "ne-NP");
    await expect(clinician.getByRole("combobox", { name: "पूर्वनिर्धारित भाषा" })).toHaveCount(0);
    await clinician.goto(SECURITY);
    await expect(clinician.getByRole("option", { name: "प्रणालीको पूर्वनिर्धारित (संस्था: नेपाली)" })).toHaveCount(1);
    await other.close();
  } finally {
    await page.goto(CLINICAL);
    const back = page.getByRole("region", { name: /^(Default language for this organisation|यो संस्थाको पूर्वनिर्धारित भाषा)$/ });
    await back.getByRole("combobox").selectOption("en");
    await back.getByRole("button").click();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
  }
});
