// Live TV: browse active channels (enabled providers + enabled categories),
// unified and grouped by provider. Search, a group (category) filter, favorites,
// and a recently-watched strip. Selecting a channel plays it via the configured
// backend (VLC by default) and records it as recent/last.
//
// The toolbar is rendered ONCE and only #live-results re-renders, so the search
// box never loses focus while typing.
import { api, type ActiveCategory, type Channel } from "../api";
import { inferTheme } from "../groups";
import { playChannel, matchesHdr } from "../playback";

let root: HTMLElement;
let channels: Channel[] = [];
let recent: Channel[] = [];
let activeCats: ActiveCategory[] = [];
let search = "";
let favoritesOnly = false;
let categoryId: number | null = null;
const LIMIT = 2000;

export async function renderLive(container: HTMLElement): Promise<void> {
  root = container;
  root.innerHTML = `<p class="muted" style="padding:24px">Loading channels…</p>`;
  [activeCats, recent] = await Promise.all([api.listActiveCategories(), api.listRecent(15)]);
  drawShell();
  await loadResults();
}

function drawShell(): void {
  root.innerHTML = `
    <div class="live">
      <div class="live-toolbar">
        <input class="live-search" type="search" placeholder="Search channels…" value="${esc(search)}" />
        <select class="group-select" title="Filter by group">${groupOptions()}</select>
        <label class="chk">
          <input type="checkbox" class="fav-filter" ${favoritesOnly ? "checked" : ""} />
          <span>Favorites</span>
        </label>
      </div>
      <div id="live-recent"></div>
      <div id="live-results"><p class="muted" style="padding:16px 4px">Loading…</p></div>
    </div>`;
  wireToolbar();
  renderRecent();
}

function groupOptions(): string {
  // Bucket categories by country code (or inferred theme) into <optgroup>s.
  const buckets = new Map<string, ActiveCategory[]>();
  for (const c of activeCats) {
    const label = c.country_code ?? inferTheme(c.name);
    (buckets.get(label) ?? buckets.set(label, []).get(label)!).push(c);
  }
  const labels = [...buckets.keys()].sort((a, b) =>
    a === "Ungrouped" ? 1 : b === "Ungrouped" ? -1 : a.localeCompare(b),
  );
  let html = `<option value="">All channels</option>`;
  for (const label of labels) {
    const cats = buckets.get(label)!.sort((a, b) => a.name.localeCompare(b.name));
    html += `<optgroup label="${esc(label)}">`;
    for (const c of cats) {
      html += `<option value="${c.id}" ${categoryId === c.id ? "selected" : ""}>${esc(
        c.name,
      )} (${c.channel_count})</option>`;
    }
    html += `</optgroup>`;
  }
  return html;
}

async function loadResults(): Promise<void> {
  channels = await api.listChannels({ categoryId, search, favoritesOnly, limit: LIMIT });
  renderResults();
}

function renderResults(): void {
  const el = root.querySelector<HTMLElement>("#live-results");
  if (!el) return;
  if (channels.length === 0) {
    el.innerHTML = `<p class="muted empty">No channels. ${
      favoritesOnly ? "No favorites yet." : "Try a different group, or sync/enable categories."
    }</p>`;
    return;
  }
  const capped = channels.length >= LIMIT;
  const grouped = groupByProvider(channels);
  el.innerHTML =
    grouped.map(([prov, list]) => providerSection(prov, list)).join("") +
    (capped ? `<p class="muted cap-note">Showing first ${LIMIT.toLocaleString()}. Filter or search to narrow.</p>` : "");
  wireResults();
}

function renderRecent(): void {
  const el = root.querySelector<HTMLElement>("#live-recent");
  if (!el) return;
  el.innerHTML = recent.length
    ? `<div class="recent-strip">
         <div class="strip-title">Recently watched</div>
         <div class="strip-row">
           ${recent
             .map(
               (c) => `<button class="recent-chip" data-open="${c.provider_id}:${c.stream_id}" title="${esc(c.name)}">
                 ${logo(c)}<span>${esc(c.name)}</span></button>`,
             )
             .join("")}
         </div>
       </div>`
    : "";
  wireRecent();
}

function providerSection(provider: string, list: Channel[]): string {
  return `
    <section class="chan-group">
      <div class="chan-group-head">${esc(provider)} <span class="muted">· ${list.length.toLocaleString()}</span></div>
      <div class="chan-list">${list.map(channelRow).join("")}</div>
    </section>`;
}

function channelRow(c: Channel): string {
  const meta = [c.category_name, c.country_code].filter(Boolean).join(" · ");
  return `
    <div class="chan-row" data-open="${c.provider_id}:${c.stream_id}">
      ${logo(c)}
      <div class="chan-info">
        <div class="chan-name">${c.num ? `<span class="chan-num">${c.num}</span>` : ""}${esc(c.name)}</div>
        ${meta ? `<div class="chan-meta muted">${esc(meta)}</div>` : ""}
      </div>
      <button class="fav-star ${c.favorite ? "on" : ""}" data-fav="${c.provider_id}:${c.stream_id}" title="Favorite">
        ${c.favorite ? "★" : "☆"}
      </button>
    </div>`;
}

function logo(c: Channel): string {
  if (c.logo) {
    return `<img class="chan-logo" src="${esc(c.logo)}" loading="lazy" onerror="this.style.visibility='hidden'" />`;
  }
  return `<span class="chan-logo placeholder">▶</span>`;
}

function groupByProvider(list: Channel[]): [string, Channel[]][] {
  const map = new Map<string, Channel[]>();
  for (const c of list) {
    (map.get(c.provider_name) ?? map.set(c.provider_name, []).get(c.provider_name)!).push(c);
  }
  return [...map.entries()];
}

// --- events ------------------------------------------------------------------

function wireToolbar(): void {
  const s = root.querySelector<HTMLInputElement>(".live-search");
  let t: number | undefined;
  s?.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(() => {
      search = s.value;
      void loadResults();
    }, 200);
  });

  root.querySelector<HTMLSelectElement>(".group-select")?.addEventListener("change", (e) => {
    const v = (e.target as HTMLSelectElement).value;
    categoryId = v ? Number(v) : null;
    void loadResults();
  });

  root.querySelector<HTMLInputElement>(".fav-filter")?.addEventListener("change", (e) => {
    favoritesOnly = (e.target as HTMLInputElement).checked;
    void loadResults();
  });
}

function wireResults(): void {
  root.querySelectorAll<HTMLElement>("#live-results [data-fav]").forEach((b) =>
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      void onFav(b);
    }),
  );
  root.querySelectorAll<HTMLElement>("#live-results [data-open]").forEach((b) =>
    b.addEventListener("click", () => void onOpen(b.dataset.open!)),
  );
}

function wireRecent(): void {
  root.querySelectorAll<HTMLElement>("#live-recent [data-open]").forEach((b) =>
    b.addEventListener("click", () => void onOpen(b.dataset.open!)),
  );
}

function parseRef(ref: string): { providerId: number; streamId: number } {
  const [p, s] = ref.split(":");
  return { providerId: Number(p), streamId: Number(s) };
}

async function onFav(btn: HTMLElement): Promise<void> {
  const { providerId, streamId } = parseRef(btn.dataset.fav!);
  const ch = channels.find((c) => c.provider_id === providerId && c.stream_id === streamId);
  const next = !(ch?.favorite ?? false);
  await api.setFavorite(providerId, streamId, next);
  if (ch) ch.favorite = next;
  btn.classList.toggle("on", next);
  btn.textContent = next ? "★" : "☆";
  if (favoritesOnly && !next) void loadResults(); // dropped from a favorites-only view
}

async function onOpen(ref: string): Promise<void> {
  const { providerId, streamId } = parseRef(ref);
  const ch =
    channels.find((c) => c.provider_id === providerId && c.stream_id === streamId) ??
    recent.find((c) => c.provider_id === providerId && c.stream_id === streamId);
  try {
    const hdr = matchesHdr(`${ch?.name ?? ""} ${ch?.category_name ?? ""}`);
    await playChannel({ providerId, streamId, name: ch?.name ?? "Live", hdr });
    await api.recordRecent(providerId, streamId);
    await api.setSetting("last_channel", ref);
    recent = await api.listRecent(15);
    renderRecent();
  } catch (e) {
    toast(String(e), true);
  }
}

function toast(msg: string, err = false): void {
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
