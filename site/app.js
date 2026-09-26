// Landing page behaviour: load the real example files, highlight them, render
// the Mermaid diagram in the page, and wire the tabs and copy buttons.
import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";

const dark = matchMedia("(prefers-color-scheme: dark)");
const mono = getComputedStyle(document.documentElement).getPropertyValue("--mono");
mermaid.initialize({
  startOnLoad: false,
  theme: dark.matches ? "dark" : "neutral",
  fontFamily: mono,
  er: { useMaxWidth: false, layoutDirection: "TB", fontSize: 13 },
});

const files = new Map();
async function load(path) {
  if (!files.has(path)) {
    files.set(path, fetch(path).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${path}: ${r.status}`)))));
  }
  return files.get(path);
}

// --- minimal highlighter: comments, strings, numbers, keywords -------------
const KEYWORDS = {
  sql: /\b(CREATE|ALTER|TABLE|VIEW|MATERIALIZED|TYPE|DOMAIN|SCHEMA|EXTENSION|SEQUENCE|INDEX|UNIQUE|PRIMARY|FOREIGN|KEY|REFERENCES|CONSTRAINT|CHECK|EXCLUDE|USING|ON|DELETE|UPDATE|SET|NULL|NOT|DEFAULT|AS|ENUM|IDENTITY|GENERATED|ALWAYS|BY|STORED|IF|EXISTS|WITH|WHERE|SELECT|FROM|JOIN|AND|OR|IS|IN|PARTITION|OF|FOR|VALUES|TO|ROW|LEVEL|SECURITY|POLICY|ENABLE|FORCE|COMMENT|COLUMN|CASCADE|RESTRICT|OWNED|ADD|DEFERRABLE|INITIALLY|DEFERRED|UNLOGGED|NO|DATA|GROUP|BEGIN|END|EXCEPTION|WHEN|THEN|ROLE|NOLOGIN|DESC|ANY|ARRAY|RANGE|DO)\b/g,
  dbml: /\b(Table|Enum|Ref|TableGroup|Note|indexes|pk|not null|null|unique|increment|default|note|name|delete|update|cascade|restrict|set null)\b/g,
  mermaid: /\b(erDiagram|PK|FK|UK)\b/g,
  json: /\b(true|false|null)\b/g,
};
function escape(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
// Matched pieces are swapped for placeholders so a later pass cannot re-match
// inside an earlier token (a keyword inside a string, say).
function highlight(text, lang) {
  const tokens = [];
  const stash = (cls, str) => `@@T${tokens.push(`<span class="${cls}">${escape(str)}</span>`) - 1}@@`;
  let out = text
    .replace(/(--|\/\/)[^\n]*/g, (m) => stash("c", m))
    .replace(/'(?:[^'\\]|\\.|'')*'|`[^`]*`/g, (m) => stash("s", m));
  if (lang === "json") {
    out = out.replace(/"(?:[^"\\]|\\.)*"(?=\s*:)/g, (m) => stash("k", m)).replace(/"(?:[^"\\]|\\.)*"/g, (m) => stash("s", m));
  } else {
    out = out.replace(/"(?:[^"\\]|\\.)*"/g, (m) => stash("t", m));
  }
  out = out.replace(/\b\d+(\.\d+)?\b/g, (m) => stash("n", m));
  if (KEYWORDS[lang]) out = out.replace(KEYWORDS[lang], (m) => stash("k", m));
  return escape(out).replace(/@@T(\d+)@@/g, (_, i) => tokens[i]);
}

// --- code panes -----------------------------------------------------------
for (const pre of document.querySelectorAll("pre.code[data-src]")) {
  load(pre.dataset.src)
    .then((text) => { pre.innerHTML = highlight(text, pre.dataset.lang); })
    .catch((err) => { pre.textContent = String(err); });
}

// --- tabs -----------------------------------------------------------------
const tabs = [...document.querySelectorAll('[role="tab"]')];
for (const tab of tabs) {
  tab.addEventListener("click", () => {
    for (const t of tabs) t.setAttribute("aria-selected", String(t === tab));
    for (const panel of document.querySelectorAll("[data-panel]")) {
      panel.hidden = panel.dataset.panel !== tab.dataset.tab;
    }
  });
}

// --- mermaid: a canvas you navigate -----------------------------------------
const diagram = document.getElementById("diagram");
const stage = document.getElementById("stage");
const source = document.getElementById("mermaid-source");
const readout = document.getElementById("zoom-level");

const MIN = 0.15;
const MAX = 6;
const clamp = (n, lo, hi) => Math.min(Math.max(n, lo), hi);

let scale = 1;
let x = 0;
let y = 0;
// A fit is only automatic until the reader takes over; after that, moving the
// diagram out from under them on a resize would be rude.
let steered = false;
let needsFit = false;

function apply() {
  stage.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  readout.textContent = `${Math.round(scale * 100)}%`;
}

/// Zoom about a point, in coordinates relative to the viewport's top left, so
/// whatever is under the cursor stays under it.
function zoomAt(px, py, factor) {
  const next = clamp(scale * factor, MIN, MAX);
  const ratio = next / scale;
  x = px - (px - x) * ratio;
  y = py - (py - y) * ratio;
  scale = next;
  apply();
}

function fit() {
  const svg = stage.querySelector("svg");
  const box = diagram.getBoundingClientRect();
  // Hidden behind the source view: nothing to measure, so fit on the way back.
  if (!svg || box.width === 0) {
    needsFit = true;
    return;
  }
  scale = 1;
  x = 0;
  y = 0;
  apply();
  const drawn = svg.getBoundingClientRect();
  if (!drawn.width || !drawn.height) return;
  const margin = 28;
  // Never magnify past 100%: a small schema centred is better than a blurry one.
  scale = clamp(
    Math.min((box.width - margin * 2) / drawn.width, (box.height - margin * 2) / drawn.height, 1),
    MIN,
    MAX,
  );
  x = (box.width - drawn.width * scale) / 2;
  y = (box.height - drawn.height * scale) / 2;
  needsFit = false;
  apply();
}

function steer() {
  steered = true;
  diagram.classList.add("touched");
}

// --- pointer gestures: drag to pan, two fingers to pan and pinch -------------
const pointers = new Map();
let previous = null;

function gesture() {
  const points = [...pointers.values()];
  const midX = points.reduce((sum, p) => sum + p.x, 0) / points.length;
  const midY = points.reduce((sum, p) => sum + p.y, 0) / points.length;
  const spread = points.length > 1 ? Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y) : 0;
  return { midX, midY, spread, count: points.length };
}

diagram.addEventListener("pointerdown", (event) => {
  if (event.pointerType === "mouse" && event.button !== 0) return;
  if (event.target.closest(".zoom")) return;
  diagram.setPointerCapture(event.pointerId);
  pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  previous = null;
  diagram.classList.add("grabbing");
});

diagram.addEventListener("pointermove", (event) => {
  if (!pointers.has(event.pointerId)) return;
  pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  const now = gesture();
  // A changed finger count re-baselines instead of jumping.
  if (previous && previous.count === now.count) {
    x += now.midX - previous.midX;
    y += now.midY - previous.midY;
    if (now.count > 1 && previous.spread > 0) {
      const box = diagram.getBoundingClientRect();
      zoomAt(now.midX - box.left, now.midY - box.top, now.spread / previous.spread);
    } else {
      apply();
    }
    steer();
  }
  previous = now;
});

// No pointerleave: the pointer is captured, so a drag that wanders outside the
// canvas still ends on pointerup, and leaving mid-drag must not drop it.
for (const type of ["pointerup", "pointercancel"]) {
  diagram.addEventListener(type, (event) => {
    pointers.delete(event.pointerId);
    previous = null;
    if (pointers.size === 0) diagram.classList.remove("grabbing");
  });
}

// Plain wheel is left to the page: a tall canvas that swallowed it would trap
// the reader. Ctrl or Command is also what a trackpad pinch sends.
diagram.addEventListener(
  "wheel",
  (event) => {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    const box = diagram.getBoundingClientRect();
    zoomAt(event.clientX - box.left, event.clientY - box.top, Math.exp(-event.deltaY * 0.01));
    steer();
  },
  { passive: false },
);

diagram.addEventListener("dblclick", (event) => {
  if (event.target.closest(".zoom")) return;
  const box = diagram.getBoundingClientRect();
  zoomAt(event.clientX - box.left, event.clientY - box.top, 1.6);
  steer();
});

diagram.addEventListener("keydown", (event) => {
  const step = event.shiftKey ? 120 : 40;
  const pans = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
  const box = diagram.getBoundingClientRect();
  if (pans[event.key]) {
    x += pans[event.key][0];
    y += pans[event.key][1];
    apply();
  } else if (event.key === "+" || event.key === "=") {
    zoomAt(box.width / 2, box.height / 2, 1.25);
  } else if (event.key === "-") {
    zoomAt(box.width / 2, box.height / 2, 0.8);
  } else if (event.key === "0") {
    fit();
    return;
  } else {
    return;
  }
  event.preventDefault();
  steer();
});

for (const button of document.querySelectorAll("[data-zoom]")) {
  button.addEventListener("click", () => {
    const box = diagram.getBoundingClientRect();
    if (button.dataset.zoom === "fit") {
      steered = false;
      diagram.classList.add("touched");
      fit();
      return;
    }
    zoomAt(box.width / 2, box.height / 2, button.dataset.zoom === "in" ? 1.25 : 0.8);
    steer();
  });
}

let resizeTimer;
addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    if (!steered) fit();
  }, 150);
});

// --- variant + diagram/source toggle ----------------------------------------
let variant = "schema.mmd";
let renderId = 0;
async function renderDiagram() {
  const id = ++renderId;
  const failure = diagram.querySelector(".error");
  if (failure) failure.remove();
  try {
    const text = await load(`examples/blog/out/${variant}`);
    source.innerHTML = highlight(text, "mermaid");
    const { svg } = await mermaid.render(`er-${id}`, text);
    if (id !== renderId) return;
    stage.innerHTML = svg;
    steered = false;
    fit();
  } catch (err) {
    stage.innerHTML = "";
    diagram.insertAdjacentHTML("beforeend", `<p class="error">${escape(String(err))}</p>`);
  }
}
for (const button of document.querySelectorAll("[data-variant]")) {
  button.addEventListener("click", () => {
    for (const b of document.querySelectorAll("[data-variant]")) b.setAttribute("aria-pressed", String(b === button));
    variant = button.dataset.variant;
    renderDiagram();
  });
}
for (const button of document.querySelectorAll("[data-view]")) {
  button.addEventListener("click", () => {
    for (const b of document.querySelectorAll("[data-view]")) b.setAttribute("aria-pressed", String(b === button));
    const showSource = button.dataset.view === "source";
    source.hidden = !showSource;
    diagram.hidden = showSource;
    if (!showSource && needsFit) fit();
  });
}
dark.addEventListener("change", () => location.reload());
renderDiagram();

// --- copy buttons ---------------------------------------------------------
for (const button of document.querySelectorAll(".copy")) {
  button.addEventListener("click", async () => {
    const text = button.dataset.copySrc
      ? await load(button.dataset.copySrc)
      : document.querySelector(button.dataset.copy).textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = "Copied";
      button.classList.add("done");
      setTimeout(() => { button.textContent = "Copy"; button.classList.remove("done"); }, 1400);
    } catch {
      button.textContent = "Select & copy";
    }
  });
}
