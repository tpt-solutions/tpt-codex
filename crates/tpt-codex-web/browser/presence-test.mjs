// Two demo pages joined to one relay: a caret set on one page is drawn on the other. Starts the
// relay binary on a local port, and serves the pages the same way browser-test.mjs does.
import { chromium } from "playwright-core";
import { spawn } from "node:child_process";
import http from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(process.env.WEB_DIR ?? path.join(HERE, "../../../target/web"));
const RELAY = path.resolve(
  process.env.RELAY_BIN ??
    path.join(HERE, "../../../target/debug", process.platform === "win32" ? "tpt-codex-relay.exe" : "tpt-codex-relay"),
);
const PORT = Number(process.env.RELAY_PORT ?? 18787);
const WEB_PORT = Number(process.env.WEB_PORT ?? 8799);
const ORIGIN = `http://localhost:${WEB_PORT}`;
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok, detail });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? " — " + detail : ""}`);
}

// Start the relay and wait until it reports that it is listening.
const relay = spawn(RELAY, [`127.0.0.1:${PORT}`], { stdio: ["ignore", "ignore", "pipe"] });
const relayReady = new Promise((resolve, reject) => {
  relay.stderr.on("data", (chunk) => {
    if (String(chunk).includes("listening")) resolve();
  });
  relay.on("error", reject);
  relay.on("exit", (code) => reject(new Error(`relay exited with ${code}`)));
});
await relayReady;

const browser = await chromium.launch({
  ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}),
  headless: true,
});
const context = await browser.newContext();
const errors = [];
// A real local server, so the pages are local origins: Chrome allows them to reach the relay.
const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, ORIGIN);
  const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
  try {
    const body = await readFile(path.join(WEB, file));
    res.writeHead(200, { "content-type": TYPES[path.extname(file)] ?? "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end("not found");
  }
});
await new Promise((resolve) => server.listen(WEB_PORT, "127.0.0.1", resolve));

async function openPage(replica) {
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("response", (r) => { if (r.status() === 404) errors.push(`404 ${r.url()}`); });
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  const query = `relay=ws://127.0.0.1:${PORT}&doc=presence-browser&replica=${replica}`;
  await page.goto(`${ORIGIN}/index.html?${query}`);
  await page.waitForSelector("#app h1", { timeout: 20000 });
  return page;
}

try {
  const alice = await openPage(1);
  const bob = await openPage(2);
  // Give both pages time to join the document before anyone moves a caret.
  await bob.waitForTimeout(500);

  check("no caret is drawn before anyone moves one", (await bob.locator(".remote-cursor").count()) === 0);

  // Alice clicks into the paragraph: her caret moves there.
  await alice.getByText("Plain, bold, and a link.").click();
  let drawn = true;
  try {
    await bob.waitForSelector(".remote-cursor", { timeout: 5000 });
  } catch {
    drawn = false;
  }
  check("Alice's caret is drawn on Bob's page", drawn);
  if (drawn) {
    const label = (await bob.locator(".remote-cursor").first().textContent()) ?? "";
    check("the drawn caret is named for Alice's replica", label.includes("Replica 1"), `label: ${label.trim()}`);
  }

  // Alice clicks the heading. Headings are drawn as plain text, so no caret shows there, but
  // Bob's paragraph no longer shows Alice's caret.
  await alice.getByText("Field notes").click();
  await bob.waitForTimeout(400);
  const paragraphCarets = await bob.locator(".text-block", { hasText: "Plain, bold" }).locator(".remote-cursor").count();
  check("moving the caret away removes it from Bob's paragraph", paragraphCarets === 0, `paragraph ${paragraphCarets}`);

  // Back in the paragraph: one caret again, not a copy.
  await alice.getByText("Plain, bold, and a link.").click();
  await bob.waitForTimeout(400);
  check("the caret returns, once", (await bob.locator(".remote-cursor").count()) === 1);

  check("no page errors", errors.length === 0, errors.join(" | "));
} finally {
  await browser.close();
  server.close();
  relay.kill();
}

const failed = results.filter((r) => !r.ok).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
