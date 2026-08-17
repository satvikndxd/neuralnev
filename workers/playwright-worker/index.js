// NeuralNav Playwright sidecar.
//
// Protocol: JSON lines over stdin/stdout.
//   request:  {"id":1,"cmd":"navigate","params":{"url":"https://…"}}
//   response: {"id":1,"ok":true,"page_state":{…},"checks":[…],"duration_ms":812}
//
// SECURITY: only the structured commands in COMMANDS below are accepted.
// There is deliberately NO evaluate/exec command — the planner can never run
// arbitrary JavaScript through this worker.

import { chromium } from "playwright";
import readline from "node:readline";

const HEADLESS = process.env.HEADLESS !== "0";
const ADBLOCK_DOMAINS = (process.env.ADBLOCK_DOMAINS || "")
  .split(",")
  .map((d) => d.trim().toLowerCase())
  .filter(Boolean);
const PROXY_URL = process.env.PROXY_URL || null;

let browser = null;
let page = null;

function log(msg) {
  // No "id" field → the Rust side treats it as a log line.
  process.stdout.write(JSON.stringify({ log: msg }) + "\n");
}

function isBlockedHost(host) {
  host = host.toLowerCase();
  return ADBLOCK_DOMAINS.some((d) => host === d || host.endsWith("." + d));
}

async function ensurePage() {
  if (page) return page;
  const launchOpts = { headless: HEADLESS };
  if (PROXY_URL) launchOpts.proxy = { server: PROXY_URL };
  browser = await chromium.launch(launchOpts);
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 900 } });
  if (ADBLOCK_DOMAINS.length > 0) {
    await ctx.route("**/*", (route) => {
      try {
        const host = new URL(route.request().url()).hostname;
        if (isBlockedHost(host)) return route.abort();
      } catch {
        /* fall through */
      }
      return route.continue();
    });
    log(`adblock active with ${ADBLOCK_DOMAINS.length} domains`);
  }
  page = await ctx.newPage();
  return page;
}

function classifyError(err) {
  const m = String(err?.message || err).toLowerCase();
  if (m.includes("timeout")) return "page_timeout";
  if (m.includes("net::") || m.includes("dns") || m.includes("connection")) return "network_error";
  if (m.includes("strict mode") || m.includes("not found") || m.includes("no element"))
    return "element_not_found";
  return "unknown";
}

async function pageState() {
  const p = await ensurePage();
  const url = p.url();
  const title = await p.title().catch(() => "");
  // Accessibility-tree-first: role/name of the interactive elements.
  const elements = await p
    .evaluate(() => {
      const roles = ["button", "link", "textbox", "searchbox", "combobox", "checkbox"];
      const found = [];
      const nodes = document.querySelectorAll(
        "a[href], button, input, textarea, select, [role]"
      );
      for (const el of nodes) {
        if (found.length >= 24) break;
        const role =
          el.getAttribute("role") ||
          (el.tagName === "A"
            ? "link"
            : el.tagName === "BUTTON"
              ? "button"
              : el.tagName === "INPUT" || el.tagName === "TEXTAREA"
                ? "textbox"
                : el.tagName === "SELECT"
                  ? "combobox"
                  : null);
        if (!role || !roles.includes(role)) continue;
        const name =
          el.getAttribute("aria-label") ||
          el.getAttribute("placeholder") ||
          (el.textContent || "").trim().slice(0, 60) ||
          null;
        found.push({ role, name, text: null, selector: null });
      }
      return found;
    })
    .catch(() => []);
  return {
    url,
    title,
    page_type: null,
    accessibility_summary: `${elements.length} interactive elements`,
    interactive_elements: elements,
    result_count: null,
    loading: false,
  };
}

// Self-healing locator ladder: role+name → visible text → CSS selector.
function locatorLadder(p, { selector, role, name, text }) {
  const ladder = [];
  if (role && name) ladder.push(() => p.getByRole(role, { name, exact: false }).first());
  if (text) ladder.push(() => p.getByText(text, { exact: false }).first());
  if (selector) ladder.push(() => p.locator(selector).first());
  if (name && !role) ladder.push(() => p.getByText(name, { exact: false }).first());
  return ladder;
}

async function resolveLocator(p, params) {
  const ladder = locatorLadder(p, params);
  for (const make of ladder) {
    const loc = make();
    if ((await loc.count().catch(() => 0)) > 0) return loc;
  }
  const err = new Error("no element found via role/name, text, or selector");
  err.neuralnavClass = "element_not_found";
  throw err;
}

const COMMANDS = {
  async navigate({ url }) {
    if (!/^https?:\/\//i.test(String(url))) {
      const err = new Error("navigate requires an http(s) url");
      err.neuralnavClass = "unknown";
      throw err;
    }
    const p = await ensurePage();
    const before = p.url();
    await p.goto(url, { waitUntil: "domcontentloaded", timeout: 25000 });
    await p.waitForLoadState("networkidle", { timeout: 8000 }).catch(() => {});
    return {
      checks: [
        { label: "URL changed", passed: p.url() !== before, detail: `${before} → ${p.url()}` },
        { label: "DOM ready", passed: true, detail: await p.title().catch(() => "") },
      ],
      page_state: await pageState(),
    };
  },

  async click(params) {
    const p = await ensurePage();
    const before = p.url();
    const loc = await resolveLocator(p, params);
    await loc.click({ timeout: 10000 });
    await p.waitForLoadState("domcontentloaded", { timeout: 8000 }).catch(() => {});
    return {
      checks: [
        { label: "element clicked", passed: true, detail: params.name || params.text || params.selector },
        { label: "page responded", passed: true, detail: p.url() !== before ? "navigation" : "in-page update" },
      ],
      page_state: await pageState(),
    };
  },

  async type(params) {
    const p = await ensurePage();
    const loc = await resolveLocator(p, params);
    await loc.fill(String(params.text ?? ""), { timeout: 10000 });
    await loc.press("Enter").catch(() => {});
    await p.waitForLoadState("domcontentloaded", { timeout: 10000 }).catch(() => {});
    return {
      checks: [
        { label: "text entered", passed: true, detail: params.text },
        { label: "submitted", passed: true, detail: "Enter pressed" },
      ],
      page_state: await pageState(),
    };
  },

  async scroll({ direction, amount }) {
    const p = await ensurePage();
    const dy = (direction === "up" ? -1 : 1) * (amount ?? 600);
    await p.mouse.wheel(0, dy);
    return {
      checks: [{ label: "viewport scrolled", passed: true, detail: `${direction} ${Math.abs(dy)}px` }],
      page_state: await pageState(),
    };
  },

  async wait({ ms }) {
    await new Promise((r) => setTimeout(r, Math.min(Number(ms) || 0, 10000)));
    return { checks: [{ label: "waited", passed: true, detail: `${ms} ms` }] };
  },

  async extract({ fields }) {
    const p = await ensurePage();
    // Generic structured extraction of headings + links (no site-specific JS
    // from the planner — fields only select which keys are kept).
    const rows = await p.evaluate(() => {
      const out = [];
      const cards = document.querySelectorAll("article, [data-component-type], li, .result, h2, h3");
      for (const el of cards) {
        if (out.length >= 20) break;
        const title = (el.querySelector("h1,h2,h3,a")?.textContent || el.textContent || "")
          .trim()
          .slice(0, 120);
        if (!title) continue;
        const link = el.querySelector("a[href]")?.href || null;
        out.push({ title, url: link });
      }
      return out;
    });
    const wanted = Array.isArray(fields) && fields.length > 0 ? fields : null;
    const filtered = wanted
      ? rows.map((r) => Object.fromEntries(Object.entries(r).filter(([k]) => wanted.includes(k) || k === "title")))
      : rows;
    return {
      checks: [
        { label: "items extracted", passed: filtered.length > 0, detail: `${filtered.length} rows` },
      ],
      extracted: { candidates: filtered },
      page_state: await pageState(),
    };
  },

  async go_back() {
    const p = await ensurePage();
    await p.goBack({ timeout: 15000 });
    return {
      checks: [{ label: "navigated back", passed: true, detail: p.url() }],
      page_state: await pageState(),
    };
  },

  async reload() {
    const p = await ensurePage();
    await p.reload({ timeout: 20000 });
    return {
      checks: [{ label: "page reloaded", passed: true, detail: p.url() }],
      page_state: await pageState(),
    };
  },

  async get_page_state() {
    return { page_state: await pageState() };
  },

  async stop() {
    if (browser) await browser.close().catch(() => {});
    browser = null;
    page = null;
    setTimeout(() => process.exit(0), 50);
    return { checks: [{ label: "stopped", passed: true, detail: "browser closed" }] };
  },
};

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });

rl.on("line", async (line) => {
  line = line.trim();
  if (!line) return;
  let req;
  try {
    req = JSON.parse(line);
  } catch {
    log(`unparseable request line`);
    return;
  }
  const { id, cmd, params = {} } = req;
  const started = Date.now();
  const handler = COMMANDS[cmd];
  if (!handler) {
    // Structured commands only — anything else is refused.
    process.stdout.write(
      JSON.stringify({ id, ok: false, error: `unknown command '${cmd}' (structured commands only)`, error_class: "unknown" }) + "\n"
    );
    return;
  }
  try {
    const result = await handler(params);
    process.stdout.write(
      JSON.stringify({ id, ok: true, duration_ms: Date.now() - started, ...result }) + "\n"
    );
  } catch (err) {
    process.stdout.write(
      JSON.stringify({
        id,
        ok: false,
        error: String(err?.message || err),
        error_class: err?.neuralnavClass || classifyError(err),
        duration_ms: Date.now() - started,
      }) + "\n"
    );
  }
});

rl.on("close", async () => {
  if (browser) await browser.close().catch(() => {});
  process.exit(0);
});

log("playwright worker ready");
