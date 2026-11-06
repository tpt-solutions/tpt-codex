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
check("full screen button marks the presenter stage in the editor", (await page.locator(".presenter.fullscreen").count()) === 1);
// Full screen covers the rest of the page, so leave it before using the controls.
await page.evaluate(() => document.fullscreenElement && document.exitFullscreen().catch(() => {}));
await page.waitForTimeout(100);
await page.getByRole("button", { name: "Next" }).click();
check("next keeps the last slide", (await page.locator(".presenter-count").textContent()) === "slide 1 of 1");
await page.getByRole("button", { name: "Stop presenting" }).click();
await page.waitForTimeout(150);
check("stop returns to the document", (await page.locator(".presenter").count()) === 0);

// Selecting a word with the keyboard and typing replaces it.
await page.locator(".para, textarea.paragraph-input").first().click();
const para2 = page.getByRole("textbox", { name: "paragraph" });
await para2.click();
await page.keyboard.press("Home");
for (let i = 0; i < 7; i++) await page.keyboard.press("ArrowRight"); // before "bold"
for (let i = 0; i < 4; i++) await page.keyboard.press("Shift+ArrowRight"); // select "bold"
await page.keyboard.type("BOLD");
await page.waitForTimeout(200);
const replaced = await page.getByRole("textbox", { name: "paragraph" }).inputValue().catch(() => "");
check("typing over a keyboard selection replaces it", replaced.startsWith("Plain, BOLD, and a link."), `value=${JSON.stringify(replaced)}`);

// An input-method composition (romaji converted to kana/kanji) commits into the paragraph.
const ime = await page.context().newCDPSession(page);
await page.getByRole("textbox", { name: "paragraph" }).click();
await page.keyboard.press("End");
await ime.send("Input.imeSetComposition", { text: "nihon", selectionStart: 5, selectionEnd: 5 });
await page.waitForTimeout(100);
await ime.send("Input.imeSetComposition", { text: "日本", selectionStart: 2, selectionEnd: 2 });
await ime.send("Input.insertText", { text: "日本" });
await page.waitForTimeout(250);
const composed = await page.getByRole("textbox", { name: "paragraph" }).inputValue().catch(() => "");
check("an IME composition commits into the paragraph", composed.endsWith("日本") && !composed.includes("nihon"), `value=${JSON.stringify(composed)}`);

// The pages view shows the document as pages, and goes back to editing.
await page.getByRole("button", { name: "Pages" }).click();
await page.waitForTimeout(150);
check("pages view draws pages", (await page.locator(".page").count()) >= 1);
await page.getByRole("button", { name: "Back to editing" }).click();
await page.waitForTimeout(150);
check("back to editing restores the editor", (await page.locator(".page").count()) === 0 && (await page.getByRole("button", { name: "Insert table" }).count()) === 1);

// Lists: insert one, type into its item, add an item, number the list, and remove an item.
await page.getByRole("button", { name: "Insert list" }).click();
await page.waitForTimeout(150);
check("inserting a list draws one item", (await page.locator(".list-item").count()) === 1);
await page.getByRole("textbox", { name: "paragraph" }).click();
await page.keyboard.type("milk");
await page.getByRole("button", { name: "Add item" }).click();
await page.getByRole("textbox", { name: "paragraph" }).click();
await page.keyboard.type("eggs");
await page.waitForTimeout(150);
// The item being typed in is a text area; the others show their text. Read both.
const itemTexts = async () =>
  page.locator(".list-item").evaluateAll((rows) =>
    rows.map((row) => {
      const area = row.querySelector("textarea");
      return area ? area.value : row.textContent.replace("Remove item", "");
    }),
  );
const items = await itemTexts();
check("add item adds an item the caret is in", items.length === 2 && items[0] === "milk" && items[1] === "eggs", JSON.stringify(items));
await page.getByRole("button", { name: "Make numbered" }).click();
await page.waitForTimeout(150);
const numbered = await page.locator(".list-item").evaluateAll((els) => els.map((e) => e.dataset.marker));
check("toggle numbers the list", numbered[0] === "1." && numbered[1] === "2.", JSON.stringify(numbered));
await page.getByRole("button", { name: "Remove item" }).first().click();
await page.waitForTimeout(150);
check("remove item removes the first item", (await page.locator(".list-item").count()) === 1 && JSON.stringify(await itemTexts()) === '["eggs"]', JSON.stringify(await itemTexts()));
await page.getByRole("button", { name: "Undo" }).click();
await page.waitForTimeout(150);
check("undo brings the removed item back", (await page.locator(".list-item").count()) === 2);

// Query builder: insert a query, set its form, apply it, and see the matching rows.
await page.getByRole("button", { name: "Insert query" }).click();
await page.waitForTimeout(150);
const querySource = page.locator(".query-source");
const before = await querySource.inputValue();
await page.getByRole("combobox", { name: "query field" }).selectOption("content");
await page.getByRole("combobox", { name: "query test" }).selectOption("not-equals");
await page.getByRole("textbox", { name: "query value" }).fill("nothing");
await page.getByRole("combobox", { name: "query sort field" }).selectOption("content");
await page.getByRole("combobox", { name: "query sort direction" }).selectOption("desc");
await page.getByRole("textbox", { name: "query limit" }).fill("2");
check("query form does not change the query until applied", (await querySource.inputValue()) === before);
await page.getByRole("button", { name: "Apply query" }).click();
await page.waitForTimeout(150);
const applied = await querySource.inputValue();
check("apply stores the query the form describes", applied !== before && applied.includes("content") && applied.includes("nothing"), applied);
check("the applied query shows matching rows", (await page.locator(".query-row").count()) >= 1);
await page.getByRole("textbox", { name: "query limit" }).fill("lots");
await page.getByRole("button", { name: "Apply query" }).click();
await page.waitForTimeout(150);
check("a bad limit is reported and the query is kept", (await page.locator(".status").first().textContent()).includes("whole number") && (await querySource.inputValue()) === applied);

// Slides: add a text box, select it, and nudge it with the arrow keys.
await page.getByRole("button", { name: "Add text" }).click();
await page.waitForTimeout(150);
check("add text draws a selectable box on the slide", (await page.locator(".slide-child").count()) === 1);
await page.locator(".slide-child").click();
await page.waitForTimeout(100);
const focused = await page.evaluate(() => document.activeElement?.className ?? "(none)");
await page.keyboard.press("ArrowRight");
await page.waitForTimeout(150);
const moved = await page.locator(".slide-child.selected").getAttribute("style");
check("arrow key nudges the selected box by one point", moved.includes("left:41px"), `focus=${focused} style=${moved}`);
await page.keyboard.press("Escape");
await page.waitForTimeout(100);
check("escape clears the slide selection", (await page.locator(".slide-child.selected").count()) === 0);

// Dragging: press on the box and move the pointer. The box moves by the pointer distance in its
// own units, which is the distance on screen divided by how big the slide is drawn. One undo step.
const readPosition = async () => {
  const style = await page.locator(".slide-child").getAttribute("style");
  const m = style.match(/left:(-?[\d.]+)px; top:(-?[\d.]+)px/);
  return { x: Number(m[1]), y: Number(m[2]), width: Number(style.match(/width:([\d.]+)px/)[1]) };
};
const startPosition = await readPosition();
const box = await page.locator(".slide-child").boundingBox();
const scale = box.width / startPosition.width;
const startX = box.x + box.width / 2;
const startY = box.y + box.height / 2;
await page.mouse.move(startX, startY);
await page.mouse.down();
await page.mouse.move(startX + 60, startY + 30, { steps: 6 });
await page.mouse.up();
await page.waitForTimeout(150);
const dragged = await readPosition();
const expectX = startPosition.x + 60 / scale;
const expectY = startPosition.y + 30 / scale;
check(
  "dragging a box moves it by the pointer distance",
  Math.abs(dragged.x - expectX) <= 2 && Math.abs(dragged.y - expectY) <= 2,
  `from (${startPosition.x}, ${startPosition.y}) to (${dragged.x}, ${dragged.y}), expected about (${expectX.toFixed(1)}, ${expectY.toFixed(1)})`,
);
await page.getByRole("button", { name: "Undo" }).click();
await page.waitForTimeout(150);
const undone = await readPosition();
check("undo puts a dragged box back in one step", undone.x === startPosition.x && undone.y === startPosition.y, JSON.stringify(undone));

// Presenter keys: with two slides, the arrow keys step through them and Escape stops.
await page.getByRole("button", { name: "Insert slide" }).click();
await page.waitForTimeout(150);
await page.getByRole("button", { name: "Present slides" }).click();
await page.waitForTimeout(150);
await page.locator(".presenter").click();
await page.keyboard.press("ArrowRight");
await page.waitForTimeout(100);
check("arrow right shows the next slide", (await page.locator(".presenter-count").textContent()) === "slide 2 of 2");
await page.keyboard.press("ArrowLeft");
await page.waitForTimeout(100);
check("arrow left shows the previous slide", (await page.locator(".presenter-count").textContent()) === "slide 1 of 2");
await page.keyboard.press("Escape");
await page.waitForTimeout(150);
check("escape stops presenting", (await page.locator(".presenter").count()) === 0);

// Export: the button downloads a PDF of the document.
{
  const downloadPromise = page.waitForEvent("download", { timeout: 20000 });
  await page.click("#export-pdf");
  const download = await downloadPromise;
  const saved = await download.path();
  const head = (await readFile(saved)).subarray(0, 5).toString("latin1");
  check("export button downloads a PDF", download.suggestedFilename() === "document.pdf" && head === "%PDF-", `${download.suggestedFilename()} ${head}`);
}

check("no page errors", errors.length === 0, errors.join(" | "));
await browser.close();

const failed = results.filter((r) => !r.ok).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
