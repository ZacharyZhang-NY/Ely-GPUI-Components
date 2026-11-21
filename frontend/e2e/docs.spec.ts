import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

/** The guides in reading order, as `src/docs/guides.ts` lists them, with their titles. */
const GUIDES: [string, string][] = [
  ["", "Introduction"],
  ["installation", "Installation"],
  ["getting-started", "Getting started"],
  ["gpui-kit", "GPUI Kit"],
  ["theme", "Theme"],
  ["focus", "Focus and keyboard"],
  ["motion", "Motion"],
  ["icons", "Icons, fonts and assets"],
  ["overlays", "Overlays"],
  ["rendering", "Rendering"],
  ["i18n", "Internationalization"],
  ["webassembly", "WebAssembly"],
  ["testing", "Testing"],
];

const path = (slug: string) => (slug ? `/docs/${slug}/` : "/docs/");

/** Collects console errors and uncaught exceptions. */
function watch(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  page.on("pageerror", (e) => errors.push(e.message));
  return errors;
}

test("the header's Docs link opens the introduction", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Docs" }).click();
  await expect(page).toHaveURL("/docs/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Introduction");
});

test("every guide loads with its title, list and no errors", async ({ page }) => {
  const errors = watch(page);
  for (const [slug, title] of GUIDES) {
    await page.goto(path(slug));
    await expect(page.locator(".doc h1")).toHaveText(title);
    await expect(page).toHaveTitle(`${title} · Docs · Ely`);
    await expect(page.locator(".prose h2").first()).toBeVisible();
  }
  expect(errors).toEqual([]);
});

test("the rail lists the guides in order and marks the open one", async ({ page }) => {
  await page.goto("/docs/theme/");
  const links = page.locator(".docs-rail a");
  await expect(links).toHaveCount(GUIDES.length);
  for (const [i, [slug, title]] of GUIDES.entries()) {
    await expect(links.nth(i)).toHaveAttribute("href", path(slug));
    await expect(links.nth(i)).toHaveText(title);
  }
  await expect(page.locator('.docs-rail a[aria-current="page"]')).toHaveText("Theme");
});

test("previous and next walk the guides", async ({ page }) => {
  await page.goto("/docs/");
  await expect(page.locator(".pager a")).toHaveCount(1);
  await page.locator(".pager a.next").click();
  await expect(page).toHaveURL("/docs/installation/");
  await page.goto(path(GUIDES[GUIDES.length - 1][0]));
  await expect(page.locator(".pager a.next")).toHaveCount(0);
});

test("an on-this-page link moves to its section", async ({ page, isMobile }) => {
  test.skip(isMobile, "the list hides on narrow screens");
  await page.goto("/docs/installation/");
  await page.locator(".doc-toc a", { hasText: "When it fails" }).click();
  await expect(page).toHaveURL("/docs/installation/#when-it-fails");
  await expect(page.locator("#when-it-fails")).toBeInViewport();
});

/** Marks the page, so a reload shows as a lost mark. */
const mark = (page: Page) => page.evaluate(() => ((window as unknown as { kept: boolean }).kept = true));
const kept = (page: Page) => page.evaluate(() => (window as unknown as { kept?: boolean }).kept === true);

test("a link in a guide routes in place", async ({ page }) => {
  await page.goto("/docs/");
  await mark(page);
  await page.locator(".prose a", { hasText: "components page" }).click();
  await expect(page).toHaveURL("/components/");
  expect(await kept(page)).toBe(true);
});

test("a link to another guide's section lands on it", async ({ page }) => {
  await page.goto("/docs/installation/");
  await mark(page);
  await page.locator(".prose a", { hasText: "first window" }).click();
  await expect(page).toHaveURL("/docs/#a-first-window");
  await expect(page.locator("#a-first-window")).toBeInViewport();
  expect(await kept(page)).toBe(true);
});

test("a code sample is the compiled example, and Copy copies it", async ({ page, context, browserName, isMobile }) => {
  test.skip(browserName !== "chromium" || isMobile, "clipboard reads need Chromium's permission");
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const file = readFileSync(new URL("../../examples/docs/hello.rs", import.meta.url), "utf8").trimEnd();
  await page.goto("/docs/");
  const block = page.locator(".code", { hasText: "examples/docs/hello.rs" });
  await expect(block.locator("code")).toHaveText(file, { useInnerText: true });
  await block.getByRole("button", { name: "Copy" }).click();
  await expect(block.getByRole("button", { name: "Copied" })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(file);
});

test("an unknown guide says so", async ({ page }) => {
  const errors = watch(page);
  await page.goto("/docs/nowhere/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Not found");
  expect(errors.join()).toContain("No guide at nowhere");
});
