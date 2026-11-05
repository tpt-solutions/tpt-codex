// Drives the tpt-codex demo page in the installed Chrome. The page is served by routing
// requests for a made-up origin to the local build folder, so no server is started.
import { chromium } from "playwright-core";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(process.env.WEB_DIR ?? path.join(HERE, "../../../target/web"));
const ORIGIN = "http://tpt-demo.test";
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? " — " + detail : ""}`);
}

const browser = await chromium.launch({
  ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}),
  headless: true,
});
const page = await browser.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });

await page.route(`${ORIGIN}/**`, async (route) => {
  const url = new URL(route.request().url());
  const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
  try {
    const body = await readFile(path.join(WEB, file));
    await route.fulfill({ body, contentType: TYPES[path.extname(file)] ?? "application/octet-stream" });
  } catch {
    await route.fulfill({ status: 404, body: "not found" });
  }
});

await page.goto(`${ORIGIN}/index.html`);
await page.waitForSelector("#app h1", { timeout: 20000 });
check("heading renders", (await page.textContent("#app h1")) === "Field notes");

// The evaluated values are in the table; the input boxes hold what was typed (B1 holds the formula).
const tableCell = (text) => page.locator("table.grid-table td", { hasText: new RegExp(`^${text}$`) });
const b1 = page.getByRole("textbox", { name: "B1" });
const a1 = page.getByRole("textbox", { name: "A1" });
check("formula cell shows 42 in the table", (await tableCell("42").count()) === 1);
check("formula cell input holds the formula", (await b1.inputValue()) === "=A1*21", `value=${await b1.inputValue()}`);

// Typing into a grid cell: change A1 to 5 and the formula's value should follow to 105.
await a1.click();
await a1.fill("5");
await a1.press("Enter");
await page.waitForTimeout(200);
check("typed cell recalculates the formula", (await tableCell("105").count()) === 1);

// Undo through the button: A1 goes back to 2 and the value to 42.
await page.getByRole("button", { name: "Undo" }).click();
await page.waitForTimeout(200);
check("undo restores the cell", (await a1.inputValue()) === "2" && (await tableCell("42").count()) === 1,
  `A1=${await a1.inputValue()}`);

// Emphasis and links are drawn as marked containers, not plain text.
const marks = await page.locator(".para .strong, .para .link").allTextContents();
check("marks are drawn in the paragraph", marks.includes("bold") && marks.includes("link"), JSON.stringify(marks));

// Focusing a paragraph turns it into a text area.
await page.locator(".para").click();
const para = page.getByRole("textbox", { name: "paragraph" });
await para.waitFor({ timeout: 5000 }).catch(() => {});
check("focused paragraph is a text area", (await para.count()) === 1);

// Typing in the text area changes the paragraph.
await para.click();
await page.keyboard.press("End");
await page.keyboard.type(" Typed here.");
await page.waitForTimeout(300);
const afterType = await page.getByRole("textbox", { name: "paragraph" }).inputValue().catch(() => "");
check("typing updates the paragraph", afterType.endsWith("Typed here."), `value=${JSON.stringify(afterType)}`);

// Leaving the paragraph shows the saved text.
await page.getByText("Field notes").click();
await page.waitForTimeout(200);
check("saved paragraph text is shown", (await page.textContent("#app")).includes("Typed here."));

// Undo removes the typed words in one step per edit.
await page.getByRole("button", { name: "Undo" }).click();
await page.waitForTimeout(200);
check("undo removes typed text", !(await page.textContent("#app")).includes("Typed here."));

// Arrow keys move the selected grid cell.
await page.getByRole("textbox", { name: "A1" }).click();
await page.waitForTimeout(150);
await page.keyboard.press("ArrowRight");
await page.waitForTimeout(150);
const selected = await page.evaluate(() => document.querySelector(".grid-cell.selected")?.getAttribute("aria-label"));
check("arrow key moves the selected cell", selected === "B1", `selected=${selected}`);

// Typing in the middle of a paragraph lands where the browser's caret is.
await page.locator(".para").click();
const mid = page.getByRole("textbox", { name: "paragraph" });
await mid.click();
await page.keyboard.press("Home");
for (let i = 0; i < 6; i++) await page.keyboard.press("ArrowRight"); // after "Plain,"
await page.keyboard.type("X");
await page.waitForTimeout(200);
const midValue = await page.getByRole("textbox", { name: "paragraph" }).inputValue().catch(() => "");
check("mid-paragraph typing inserts at the caret", midValue.startsWith("Plain,X "), `value=${JSON.stringify(midValue)}`);
await page.getByRole("button", { name: "Undo" }).click();
await page.waitForTimeout(150);

// Slides: insert one, present it, step through it, and stop.
await page.getByRole("button", { name: "Insert slide" }).click();
await page.waitForTimeout(150);
check("inserting a slide draws it", (await page.locator(".slide").count()) === 1);
await page.getByRole("button", { name: "Present slides" }).click();
await page.waitForTimeout(150);
check("presenter shows the slide", (await page.locator(".presenter-count").textContent()) === "slide 1 of 1");
// The full-screen button runs while a presenter is open, without a page error.
await page.getByRole("button", { name: "Full screen" }).click();
await page.waitForTimeout(100);
// Full screen covers the rest of the page, so leave it before using the controls.
await page.evaluate(() => document.fullscreenElement && document.exitFullscreen().catch(() => {}));
await page.waitForTimeout(100);
await page.getByRole("button", { name: "Next" }).click();
check("next keeps the last slide", (await page.locator(".presenter-count").textContent()) === "slide 1 of 1");
await page.getByRole("button", { name: "Stop presenting" }).click();
await page.waitForTimeout(150);
check("stop returns to the document", (await page.locator(".presenter").count()) === 0);

check("no page errors", errors.length === 0, errors.join(" | "));
await browser.close();

const failed = results.filter((r) => !r.ok).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
