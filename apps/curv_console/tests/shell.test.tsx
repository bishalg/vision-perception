/** WP-C.23 (N-2, N-3): the shell's top bar, sidebar, breadcrumb and drawer. */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShell } from "@/components/shell/AppShell";
import { detailLabel } from "@/components/shell/Breadcrumb";
import { navTree } from "@/lib/nav";

let pathname = "/o/clinic/sessions/S1";
vi.mock("next/navigation", () => ({ usePathname: () => pathname, useRouter: () => ({ push: vi.fn(), refresh: vi.fn() }) }));
vi.mock("@/app/login/actions", () => ({ logoutAction: vi.fn() }));

function setDrawerMode(on: boolean) {
  window.matchMedia = ((q: string) => ({ matches: on && q.includes("max-width"), media: q })) as unknown as typeof window.matchMedia;
}

const shell = (role: Parameters<typeof navTree>[1] = "owner") =>
  render(
    <AppShell
      tree={navTree("/o", role)}
      accountSecurityHref="/o/account/security"
      orgs={{ current: "o", list: [{ id: "o", name: "Org" }], allowAnyId: false }}
    >
      <h1>{"Page"}</h1>
    </AppShell>,
  );

describe("AppShell", () => {
  beforeEach(() => setDrawerMode(false));
  afterEach(() => {
    pathname = "/o/clinic/sessions/S1";
  });

  it("shows the divisions in the top bar and the current division's items in the sidebar", () => {
    shell();
    const main = screen.getAllByRole("navigation", { name: "Main navigation" })[0]!;
    expect(within(main).getAllByRole("link").map((a) => a.textContent)).toEqual(["Clinic", "Research", "Configuration", "Data", "Administration"]);
    expect(within(main).getByRole("link", { name: "Clinic" })).toHaveAttribute("aria-current", "true");
    const side = screen.getByRole("navigation", { name: "Clinic" });
    expect(within(side).getAllByRole("link").map((a) => a.textContent)).toEqual(["Overview", "Worklist", "Participants", "Follow-ups", "Review", "Sessions"]);
    expect(within(side).getByRole("link", { name: "Sessions" })).toHaveAttribute("aria-current", "page");
  });

  it("writes the breadcrumb Division › Item › Detail", () => {
    shell();
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["Clinic", "Sessions", "S1"]);
    expect(within(crumbs).getByRole("link", { name: "Sessions" })).toHaveAttribute("href", "/o/clinic/sessions");
  });

  it("has no sidebar or breadcrumb outside a division (account pages)", () => {
    pathname = "/o/account/security";
    shell();
    expect(screen.queryByRole("navigation", { name: "Breadcrumb" })).toBeNull();
    expect(document.querySelector("[data-sidebar]")).toHaveAttribute("data-sidebar", "off");
    // Account security is in the Account menu, not a division.
    expect(screen.getByRole("link", { name: "Account security", hidden: true })).toHaveAttribute("href", "/o/account/security");
  });

  it("shows a researcher the research items", () => {
    pathname = "/o/research/runs/r1";
    shell("researcher");
    const side = screen.getByRole("navigation", { name: "Research" });
    expect(within(side).getAllByRole("link").map((a) => a.textContent)).toEqual(["Experiments", "Cohorts", "Datasets", "Analysis runs"]);
    expect(within(side).getByRole("link", { name: "Analysis runs" })).toHaveAttribute("aria-current", "page");
  });

  it("opens the drawer from Menu, traps focus in it and closes on Escape (N-3)", async () => {
    setDrawerMode(true);
    const user = userEvent.setup();
    shell();
    const menu = screen.getByRole("button", { name: "Menu" });
    expect(menu).toHaveAttribute("aria-expanded", "false");
    await user.click(menu);
    expect(menu).toHaveAttribute("aria-expanded", "true");
    const drawer = document.getElementById("sidenav")!;
    expect(drawer).toHaveAttribute("data-open", "true");
    const links = within(drawer).getAllByRole("link", { hidden: true });
    expect(document.activeElement).toBe(links[0]);
    // Shift+Tab from the first focusable wraps to the last one.
    await user.tab({ shift: true });
    expect(drawer.contains(document.activeElement)).toBe(true);
    await user.keyboard("{Escape}");
    expect(drawer).toHaveAttribute("data-open", "false");
    expect(document.activeElement).toBe(menu);
  });
});

describe("detailLabel", () => {
  const item = { href: "/o/clinic/sessions", text: "Sessions" };
  it("names the detail below an item", () => {
    expect(detailLabel("/o/clinic/sessions", item, "New")).toBeNull();
    expect(detailLabel("/o/clinic/sessions/S%201", item, "New")).toBe("S 1");
    expect(detailLabel("/o/clinic/sessions/new", item, "New")).toBe("New");
    expect(detailLabel("/o/clinic/sessions/4f0e7a52-6a8c-4c3e-9a51-2f6b0c1d9e01", item, "New")).toBe("4f0e7a52…");
    expect(detailLabel("/o/clinic/sessions/S1", undefined, "New")).toBeNull();
  });
});
