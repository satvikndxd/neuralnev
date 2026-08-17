/* Latency-budget bar chart. Ordinal single-hue blue ramp (validated with the
   dataviz palette validator: monotone lightness, adjacent ΔL ≥ 0.06, light
   end ≥ 2:1 on the #faf7f0 chart surface). Bars ≤ 24px, 4px rounded data-end
   square at the baseline, hairline grid, values at the tip, hover tooltip,
   table view for accessibility. */

interface Stage {
  stage: string;
  ms: number;
}

const LATENCY: Stage[] = [
  { stage: "Voice capture + VAD", ms: 150 },
  { stage: "Streaming ASR partial", ms: 400 },
  { stage: "Intent classification", ms: 300 },
  { stage: "Action planning", ms: 500 },
  { stage: "Browser dispatch", ms: 200 },
  { stage: "First spoken feedback", ms: 150 },
];
const RAMP = ["#79aeee", "#5093e6", "#2a78d6", "#1e5dae", "#154a85", "#0c355e"];
const TOTAL = LATENCY.reduce((s, d) => s + d.ms, 0);

const NS = "http://www.w3.org/2000/svg";

function el(tag: string, attrs: Record<string, string | number>, text?: string): SVGElement {
  const n = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, String(v));
  if (text != null) n.textContent = text;
  return n;
}

export function renderLatencyChart(): void {
  const host = document.getElementById("latency-chart");
  const tip = document.getElementById("viz-tooltip");
  if (!host || !tip) return;

  const W = 640;
  const LM = 168;
  const RM = 56;
  const TM = 8;
  const BM = 26;
  const rowH = 34;
  const barH = 18;
  const r = 4;
  const H = TM + LATENCY.length * rowH + BM;
  const plotW = W - LM - RM;
  const max = 500;
  const x = (v: number) => LM + (v / max) * plotW;

  const svg = el("svg", { viewBox: `0 0 ${W} ${H}`, "aria-hidden": "true" }) as SVGSVGElement;

  for (let v = 0; v <= max; v += 100) {
    svg.appendChild(
      el("line", {
        x1: x(v), x2: x(v), y1: TM, y2: TM + LATENCY.length * rowH,
        stroke: v === 0 ? "#b9b3a0" : "#e1ddd0", "stroke-width": 1,
      }),
    );
    svg.appendChild(
      el("text", {
        x: x(v), y: H - 8, "text-anchor": "middle", "font-size": 11, fill: "#8a8474",
        style: "font-variant-numeric: tabular-nums",
      }, String(v)),
    );
  }

  LATENCY.forEach((d, i) => {
    const y = TM + i * rowH + (rowH - barH) / 2;
    const w = x(d.ms) - x(0);
    svg.appendChild(
      el("text", {
        x: LM - 10, y: y + barH / 2 + 4, "text-anchor": "end",
        "font-size": 12.5, fill: "#55503f", "font-weight": 500,
      }, d.stage),
    );
    svg.appendChild(
      el("path", {
        d: `M${x(0)},${y} h${w - r} a${r},${r} 0 0 1 ${r},${r} v${barH - 2 * r} a${r},${r} 0 0 1 ${-r},${r} h${-(w - r)} Z`,
        fill: RAMP[i],
      }),
    );
    svg.appendChild(
      el("text", {
        x: x(d.ms) + 7, y: y + barH / 2 + 4, "font-size": 12, fill: "#16130e",
        "font-weight": 600, style: "font-variant-numeric: tabular-nums",
      }, `${d.ms} ms`),
    );
    const hit = el("rect", {
      x: 0, y: TM + i * rowH, width: W, height: rowH, fill: "transparent", class: "bar-hit",
    });
    (hit as SVGElement & { dataset: DOMStringMap }).dataset.idx = String(i);
    svg.appendChild(hit);
  });

  host.appendChild(svg);

  svg.addEventListener("pointermove", (e) => {
    const t = (e.target as Element).closest(".bar-hit") as SVGElement | null;
    if (!t || !t.dataset.idx) {
      tip.hidden = true;
      return;
    }
    const d = LATENCY[Number(t.dataset.idx)];
    tip.innerHTML = `<b>${d.stage}</b><span class="tt-num">${d.ms} ms · ${Math.round((d.ms / TOTAL) * 100)}% of ${TOTAL.toLocaleString()} ms budget</span>`;
    tip.hidden = false;
    const pad = 14;
    let tx = e.clientX + pad;
    let ty = e.clientY + pad;
    const rect = tip.getBoundingClientRect();
    if (tx + rect.width > innerWidth - 8) tx = e.clientX - rect.width - pad;
    if (ty + rect.height > innerHeight - 8) ty = e.clientY - rect.height - pad;
    tip.style.left = `${tx}px`;
    tip.style.top = `${ty}px`;
  });
  svg.addEventListener("pointerleave", () => {
    tip.hidden = true;
  });

  const tbody = document.querySelector<HTMLTableSectionElement>("#latency-table tbody");
  const toggle = document.getElementById("latency-toggle") as HTMLButtonElement | null;
  const table = document.getElementById("latency-table") as HTMLTableElement | null;
  if (!tbody || !toggle || !table) return;
  for (const d of LATENCY) {
    const tr = document.createElement("tr");
    tr.innerHTML = `<td>${d.stage}</td><td>${d.ms}</td><td>${Math.round((d.ms / TOTAL) * 100)}%</td>`;
    tbody.appendChild(tr);
  }
  toggle.addEventListener("click", () => {
    const showTable = table.hidden;
    table.hidden = !showTable;
    (host as HTMLElement).style.display = showTable ? "none" : "";
    toggle.setAttribute("aria-pressed", String(showTable));
    toggle.textContent = showTable ? "Chart view" : "Table view";
  });
}
