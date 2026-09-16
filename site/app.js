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

// --- mermaid: variant + diagram/source toggle ------------------------------
const diagram = document.getElementById("diagram");
const source = document.getElementById("mermaid-source");
let variant = "schema.mmd";
let renderId = 0;
async function renderDiagram() {
  const id = ++renderId;
  try {
    const text = await load(`examples/blog/out/${variant}`);
    source.innerHTML = highlight(text, "mermaid");
    const { svg } = await mermaid.render(`er-${id}`, text);
    if (id === renderId) diagram.innerHTML = svg;
  } catch (err) {
    diagram.innerHTML = `<p class="error">${escape(String(err))}</p>`;
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
