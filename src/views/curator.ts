// Curation wizard for one provider: tame a huge multi-country channel dump by
// enabling only the countries/categories you want. Disabled groups are fully
// excluded from EPG fetching (enforced in the backend). Country rows roll up
// their categories; expand a country to toggle individual categories.
import { api, type Category, type CurationStats } from "../api";

interface Group {
  key: string; // country_code or "__other__"
  code: string | null;
  name: string;
  cats: Category[];
  channels: number;
  enabled: number;
}

let host: HTMLElement;
let providerId: number;
let providerName: string;
let categories: Category[] = [];
let stats: CurationStats | null = null;
let filter = "";
const expanded = new Set<string>();

export async function renderCurator(
  container: HTMLElement,
  pid: number,
  name: string,
): Promise<void> {
  host = container;
  providerId = pid;
  providerName = name;
  host.innerHTML = `<p class="muted">Loading categories…</p>`;
  await reload();
}

async function reload(): Promise<void> {
  [categories, stats] = await Promise.all([
    api.listCategories(providerId),
    api.curationStats(providerId),
  ]);
  draw();
}

async function refreshStats(): Promise<void> {
  stats = await api.curationStats(providerId);
  const el = host.querySelector<HTMLElement>('[data-el="stats"]');
  if (el && stats) el.innerHTML = statsText(stats);
}

function groups(): Group[] {
  const map = new Map<string, Group>();
  for (const c of categories) {
    const key = c.country_code ?? "__other__";
    let g = map.get(key);
    if (!g) {
      g = {
        key,
        code: c.country_code,
        name: c.country_name ?? "Other",
        cats: [],
        channels: 0,
        enabled: 0,
      };
      map.set(key, g);
    }
    g.cats.push(c);
    g.channels += c.channel_count;
    if (c.enabled) g.enabled += 1;
  }
  return [...map.values()].sort((a, b) => {
    if (a.code === null) return 1;
    if (b.code === null) return -1;
    return b.channels - a.channels || a.name.localeCompare(b.name);
  });
}

function statsText(s: CurationStats): string {
  return `<strong>${s.enabled_categories}</strong>/${s.total_categories} categories ·
    <strong>${s.enabled_channels.toLocaleString()}</strong>/${s.total_channels.toLocaleString()} channels enabled`;
}

function draw(): void {
  if (categories.length === 0) {
    host.innerHTML = `
      <h3>Curate — ${esc(providerName)}</h3>
      <p class="muted">No categories cached yet. Sync this provider first.</p>`;
    return;
  }

  host.innerHTML = `
    <div class="curator-head">
      <h3>Curate — ${esc(providerName)}</h3>
      <div class="curator-stats muted" data-el="stats">${stats ? statsText(stats) : ""}</div>
    </div>
    <p class="muted">Disabled countries/categories are fully excluded from EPG fetching.</p>
    <div class="curator-toolbar">
      <input class="curator-search" type="search" placeholder="Filter categories…" value="${esc(filter)}" />
      <div class="curator-bulk">
        <button class="btn" data-bulk="on">Enable all</button>
        <button class="btn" data-bulk="off">Disable all</button>
      </div>
    </div>
    <div class="curator-body">${filter ? flatList() : groupedList()}</div>
  `;
  wire();
  applyIndeterminate();
}

function groupedList(): string {
  return groups()
    .map((g) => {
      const isOpen = expanded.has(g.key);
      const full = g.enabled === g.cats.length;
      const rows = isOpen
        ? `<div class="cat-list">${g.cats.map(catRow).join("")}</div>`
        : "";
      return `
        <div class="group ${isOpen ? "open" : ""}">
          <div class="group-head">
            <label class="switch">
              <input type="checkbox" data-country="${g.code ?? "__other__"}" ${full ? "checked" : ""} />
              <span class="slider"></span>
            </label>
            <button class="group-toggle" data-expand="${g.key}">
              <span class="caret">${isOpen ? "▾" : "▸"}</span>
              <span class="group-name">${esc(g.name)}</span>
            </button>
            <span class="group-count muted">${g.channels.toLocaleString()} ch · ${g.enabled}/${g.cats.length} groups</span>
          </div>
          ${rows}
        </div>`;
    })
    .join("");
}

function flatList(): string {
  const needle = filter.toLowerCase();
  const matches = categories.filter((c) => c.name.toLowerCase().includes(needle));
  if (matches.length === 0) return `<p class="muted">No categories match “${esc(filter)}”.</p>`;
  return `<div class="cat-list flat">${matches.map(catRow).join("")}</div>`;
}

function catRow(c: Category): string {
  const flag = c.country_name ? `<span class="cat-country muted">${esc(c.country_name)}</span>` : "";
  return `
    <div class="cat-row">
      <label class="switch">
        <input type="checkbox" data-cat="${c.id}" ${c.enabled ? "checked" : ""} />
        <span class="slider"></span>
      </label>
      <span class="cat-name">${esc(c.name)}</span>
      ${flag}
      <span class="cat-count muted">${c.channel_count.toLocaleString()} ch</span>
    </div>`;
}

function applyIndeterminate(): void {
  for (const g of groups()) {
    const box = host.querySelector<HTMLInputElement>(`[data-country="${g.code ?? "__other__"}"]`);
    if (box) box.indeterminate = g.enabled > 0 && g.enabled < g.cats.length;
  }
}

// --- events ------------------------------------------------------------------

function wire(): void {
  const search = host.querySelector<HTMLInputElement>(".curator-search");
  let t: number | undefined;
  search?.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(() => {
      filter = search.value;
      draw();
    }, 140);
  });

  host.querySelectorAll<HTMLElement>("[data-bulk]").forEach((b) =>
    b.addEventListener("click", () => void onBulk(b.dataset.bulk === "on")),
  );
  host.querySelectorAll<HTMLElement>("[data-expand]").forEach((b) =>
    b.addEventListener("click", () => {
      const key = b.dataset.expand!;
      if (expanded.has(key)) expanded.delete(key);
      else expanded.add(key);
      draw();
    }),
  );
  host.querySelectorAll<HTMLInputElement>("[data-country]").forEach((box) =>
    box.addEventListener("change", () => void onCountry(box.dataset.country!, box.checked)),
  );
  host.querySelectorAll<HTMLInputElement>("[data-cat]").forEach((box) =>
    box.addEventListener("change", () => void onCategory(Number(box.dataset.cat), box.checked)),
  );
}

async function onBulk(enabled: boolean): Promise<void> {
  await api.setAllCategoriesEnabled(providerId, enabled);
  categories = categories.map((c) => ({ ...c, enabled }));
  draw();
  await refreshStats();
}

async function onCountry(code: string, enabled: boolean): Promise<void> {
  const country = code === "__other__" ? null : code;
  await api.setCountryEnabled(providerId, country, enabled);
  categories = categories.map((c) =>
    (c.country_code ?? "__other__") === code ? { ...c, enabled } : c,
  );
  draw();
  await refreshStats();
}

async function onCategory(catId: number, enabled: boolean): Promise<void> {
  await api.setCategoryEnabled(catId, enabled);
  categories = categories.map((c) => (c.id === catId ? { ...c, enabled } : c));
  // Update group header counts / indeterminate without a full redraw jump.
  draw();
  await refreshStats();
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
