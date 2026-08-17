/* Success-rate meters. Below-target categories are flagged with an icon +
   label (never color alone). */

interface Meter {
  name: string;
  rate: number;
}

const SUCCESS: Meter[] = [
  { name: "Navigation", rate: 100 },
  { name: "Search & filter", rate: 100 },
  { name: "Extraction", rate: 95 },
  { name: "Multi-step composite", rate: 85 },
];
const TARGET = 90;

export function renderMeters(): void {
  const host = document.getElementById("meters");
  if (!host) return;
  for (const d of SUCCESS) {
    const below = d.rate < TARGET;
    const row = document.createElement("div");
    row.className = "meter";
    row.innerHTML =
      `<span class="meter-name">${d.name}${below ? '<span class="warn-flag">⚠ below target</span>' : ""}</span>` +
      `<span class="meter-val">${d.rate}%</span>` +
      `<div class="meter-track${below ? " warn" : ""}" role="meter" aria-valuenow="${d.rate}" aria-valuemin="0" aria-valuemax="100" aria-label="${d.name} success rate">` +
      `<span class="meter-fill" style="width:${d.rate}%"></span>` +
      `<span class="meter-target" style="left:${TARGET}%" title="90% target"></span>` +
      `</div>`;
    host.appendChild(row);
  }
}
