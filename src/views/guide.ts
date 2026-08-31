// Guide view: a scrollable EPG timeline grid (channels × time). Pick a provider,
// refresh its EPG (curation-gated, concurrent fetch with a progress bar), and
// read the schedule. EPG auto-refreshes if the cache is older than 12h.
import { api, type Channel, type EpgProgram, type Provider } from "../api";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const HOURS = 12;
const PX_PER_MIN = 6;
const ROW_H = 46;
const MAX_ROWS = 400;
const AUTO_REFRESH_SECS = 12 * 3600;

let root: HTMLElement;
let providers: Provider[] = [];
let selected: number | null = null;
let channels: Channel[] = [];
let programs: EpgProgram[] = [];
let windowStart = 0;
let windowEnd = 0;
let progressUnlisten: UnlistenFn | null = null;
let syncing = false;

export async function renderGuide(container: HTMLElement): Promise<void> {
  root = container;
  providers = (await api.listProviders()).filter((p) => p.enabled);
  if (providers.length === 0) {
    root.innerHTML = `<div class="placeholder"><h2>Guide</h2><p class="muted">Enable a provider and sync it first.</p></div>`;
    return;
  }
  if (!selected || !providers.some((p) => p.id === selected)) selected = providers[0].id;
  computeWindow();
  await loadGrid();
  await maybeAutoRefresh();
}

function computeWindow(): void {
  const now = Math.floor(Date.now() / 1000);
  windowStart = Math.floor(now / 1800) * 1800 - 1800; // floor to 30m, back 30m
  windowEnd = windowStart + HOURS * 3600;
}

async function loadGrid(): Promise<void> {
  const [chans, epg] = await Promise.all([
    api.listChannels({ providerId: selected, limit: 0 }),
    api.getEpg(selected!, windowStart, windowEnd),
  ]);
  channels = chans;
  programs = epg;
  draw();
}

async function maybeAutoRefresh(): Promise<void> {
  if (programs.length > 0 || syncing) return;
  const last = await api.getSetting(`last_epg_sync:${selected}`);
  const stale = !last || Math.floor(Date.now() / 1000) - Number(last) > AUTO_REFRESH_SECS;
  if (stale) void doSync();
}

function draw(): void {
  const byStream = new Map<number, EpgProgram[]>();
  for (const p of programs) {
    const a = byStream.get(p.stream_id) ?? [];
    a.push(p);
    byStream.set(p.stream_id, a);
  }
  // Only channels that have programmes in the window, in catalog order.
  const rows = channels.filter((c) => byStream.has(c.stream_id)).slice(0, MAX_ROWS);
  const capped = channels.filter((c) => byStream.has(c.stream_id)).length > MAX_ROWS;
  const gridWidth = HOURS * 60 * PX_PER_MIN;

  root.innerHTML = `
    <div class="guide">
      <div class="guide-toolbar">
        <select class="prov-select">
          ${providers.map((p) => `<option value="${p.id}" ${p.id === selected ? "selected" : ""}>${esc(p.name)}</option>`).join("")}
        </select>
        <button class="btn btn-primary refresh-epg" ${syncing ? "disabled" : ""}>${syncing ? "Refreshing…" : "Refresh EPG"}</button>
        <div class="epg-progress" data-el="prog"></div>
      </div>

      ${
        rows.length === 0
          ? `<p class="muted empty">No EPG cached for this provider yet. Hit “Refresh EPG”.</p>`
          : gridHtml(rows, byStream, gridWidth)
      }
      ${capped ? `<p class="muted cap-note">Showing first ${MAX_ROWS} channels with EPG.</p>` : ""}
    </div>`;
  wire();
  syncScrollAndNow();
}

function gridHtml(
  rows: Channel[],
  byStream: Map<number, EpgProgram[]>,
  gridWidth: number,
): string {
  return `
    <div class="guide-grid">
      <div class="guide-corner"></div>
      <div class="guide-timerow-wrap">
        <div class="guide-timerow" style="width:${gridWidth}px">
          ${timeAxis()}
          <div class="now-line" data-el="nowline"></div>
        </div>
      </div>
      <div class="guide-channels">
        ${rows.map((c) => `<div class="guide-chan" style="height:${ROW_H}px">${esc(c.name)}</div>`).join("")}
      </div>
      <div class="guide-lanes-wrap" data-el="lanes">
        <div class="guide-lanes" style="width:${gridWidth}px">
          ${rows.map((c) => laneHtml(byStream.get(c.stream_id) ?? [])).join("")}
          <div class="now-line tall" data-el="nowline2"></div>
        </div>
      </div>
    </div>`;
}

function timeAxis(): string {
  let html = "";
  const firstHour = Math.ceil(windowStart / 3600) * 3600;
  for (let t = firstHour; t < windowEnd; t += 3600) {
    const left = ((t - windowStart) / 60) * PX_PER_MIN;
    const d = new Date(t * 1000);
    const label = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    html += `<div class="time-tick" style="left:${left}px">${label}</div>`;
  }
  return html;
}

function laneHtml(progs: EpgProgram[]): string {
  const blocks = progs
    .map((p) => {
      const start = Math.max(p.start_utc, windowStart);
      const end = Math.min(p.stop_utc, windowEnd);
      if (end <= start) return "";
      const left = ((start - windowStart) / 60) * PX_PER_MIN;
      const width = Math.max(2, ((end - start) / 60) * PX_PER_MIN - 2);
      const t = `${fmt(p.start_utc)}–${fmt(p.stop_utc)}`;
      return `<div class="prog" style="left:${left}px;width:${width}px"
                data-title="${esc(p.title)}" data-time="${t}" data-desc="${esc(p.description)}"
                title="${esc(p.title)} (${t})">
                <span class="prog-title">${esc(p.title)}</span>
              </div>`;
    })
    .join("");
  return `<div class="guide-lane" style="height:${ROW_H}px">${blocks}</div>`;
}

// --- events ------------------------------------------------------------------

function wire(): void {
  root.querySelector<HTMLSelectElement>(".prov-select")?.addEventListener("change", (e) => {
    selected = Number((e.target as HTMLSelectElement).value);
    void (async () => {
      computeWindow();
      await loadGrid();
      await maybeAutoRefresh();
    })();
  });

  root.querySelector<HTMLButtonElement>(".refresh-epg")?.addEventListener("click", () => void doSync());

  root.querySelectorAll<HTMLElement>(".prog").forEach((b) =>
    b.addEventListener("click", () => showDetail(b)),
  );

  // Keep channel column and time row synced with the lanes scroll.
  const lanes = root.querySelector<HTMLElement>('[data-el="lanes"]');
  const chans = root.querySelector<HTMLElement>(".guide-channels");
  const timerow = root.querySelector<HTMLElement>(".guide-timerow-wrap");
  lanes?.addEventListener("scroll", () => {
    if (chans) chans.scrollTop = lanes.scrollTop;
    if (timerow) timerow.scrollLeft = lanes.scrollLeft;
  });
}

function showDetail(b: HTMLElement): void {
  const title = b.dataset.title ?? "";
  const time = b.dataset.time ?? "";
  const desc = b.dataset.desc ?? "";
  let bar = root.querySelector<HTMLElement>(".prog-detail");
  if (!bar) {
    bar = document.createElement("div");
    bar.className = "prog-detail";
    root.querySelector(".guide")?.appendChild(bar);
  }
  bar.innerHTML = `<strong>${esc(title)}</strong> <span class="muted">${esc(time)}</span>${
    desc ? `<div class="pd-desc muted">${esc(desc)}</div>` : ""
  }`;
}

async function doSync(): Promise<void> {
  if (syncing || selected == null) return;
  syncing = true;
  draw();
  const prog = root.querySelector<HTMLElement>('[data-el="prog"]');
  progressUnlisten?.();
  progressUnlisten = await listen<{ done: number; total: number }>("epg-progress", (e) => {
    if (prog) prog.textContent = `${e.payload.done}/${e.payload.total} channels`;
  });
  try {
    const res = await api.syncEpg(selected);
    flash(`EPG updated: ${res.programs.toLocaleString()} programmes across ${res.channels_fetched} channels`);
    await loadGrid();
  } catch (err) {
    flash(`EPG refresh failed: ${err}`, true);
  } finally {
    syncing = false;
    progressUnlisten?.();
    progressUnlisten = null;
    draw();
  }
}

function syncScrollAndNow(): void {
  // Scroll so "now" is near the left, and position the now-line.
  const now = Math.floor(Date.now() / 1000);
  const nowLeft = ((now - windowStart) / 60) * PX_PER_MIN;
  root.querySelectorAll<HTMLElement>('[data-el="nowline"], [data-el="nowline2"]').forEach((l) => {
    l.style.left = `${nowLeft}px`;
  });
  const lanes = root.querySelector<HTMLElement>('[data-el="lanes"]');
  if (lanes) lanes.scrollLeft = Math.max(0, nowLeft - 80);
}

function fmt(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function flash(msg: string, err = false): void {
  const t = document.createElement("div");
  t.className = `toast ${err ? "err" : "ok"}`;
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.classList.add("show"), 10);
  setTimeout(() => {
    t.classList.remove("show");
    setTimeout(() => t.remove(), 300);
  }, 3200);
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
