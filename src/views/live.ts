// Live TV view: browse active channels (only enabled providers + enabled
// categories) unified and grouped by provider, with search, a favorites filter,
// per-channel favorite stars, and a recently-watched strip. Playback itself
// arrives in a later phase; selecting a channel records it as recent/last.
import { api, type Channel } from "../api";
import { openPlayer } from "./player";

let root: HTMLElement;
let channels: Channel[] = [];
let recent: Channel[] = [];
let search = "";
let favoritesOnly = false;
const LIMIT = 500;

export async function renderLive(container: HTMLElement): Promise<void> {
  root = container;
  root.innerHTML = `<p class="muted" style="padding:24px">Loading channels…</p>`;
  await reload();
}

async function reload(): Promise<void> {
  [channels, recent] = await Promise.all([
    api.listChannels({ search, favoritesOnly, limit: LIMIT }),
    api.listRecent(15),
  ]);
  draw();
}

async function refreshLists(): Promise<void> {
  // Lighter refresh after a favorite/select without rebuilding the toolbar.
  [channels, recent] = await Promise.all([
    api.listChannels({ search, favoritesOnly, limit: LIMIT }),
    api.listRecent(15),
  ]);
  draw();
}

function draw(): void {
  const grouped = groupByProvider(channels);
  const capped = channels.length >= LIMIT;

  root.innerHTML = `
    <div class="live">
      <div class="live-toolbar">
        <input class="live-search" type="search" placeholder="Search channels…" value="${esc(search)}" />
        <label class="chk">
          <input type="checkbox" class="fav-filter" ${favoritesOnly ? "checked" : ""} />
          <span>Favorites only</span>
        </label>
      </div>

      ${recent.length ? recentStrip() : ""}

      ${
        channels.length === 0
          ? `<p class="muted empty">No channels. ${favoritesOnly ? "No favorites yet." : "Add a provider, sync it, and enable some categories."}</p>`
          : grouped.map(([prov, list]) => providerSection(prov, list)).join("")
      }
      ${capped ? `<p class="muted cap-note">Showing first ${LIMIT}. Refine your search to narrow results.</p>` : ""}
    </div>`;
  wire();
}

function recentStrip(): string {
  return `
    <div class="recent-strip">
      <div class="strip-title">Recently watched</div>
      <div class="strip-row">
        ${recent.map((c) => `
          <button class="recent-chip" data-open="${c.provider_id}:${c.stream_id}" title="${esc(c.name)}">
            ${logo(c)}<span>${esc(c.name)}</span>
          </button>`).join("")}
      </div>
    </div>`;
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
    const arr = map.get(c.provider_name) ?? [];
    arr.push(c);
    map.set(c.provider_name, arr);
  }
  return [...map.entries()];
}

// --- events ------------------------------------------------------------------

function wire(): void {
  const s = root.querySelector<HTMLInputElement>(".live-search");
  let t: number | undefined;
  s?.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(() => {
      search = s.value;
      void reload();
    }, 200);
  });

  root.querySelector<HTMLInputElement>(".fav-filter")?.addEventListener("change", (e) => {
    favoritesOnly = (e.target as HTMLInputElement).checked;
    void reload();
  });

  root.querySelectorAll<HTMLElement>("[data-fav]").forEach((b) =>
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      void onFav(b.dataset.fav!);
    }),
  );
  root.querySelectorAll<HTMLElement>("[data-open]").forEach((b) =>
    b.addEventListener("click", () => void onOpen(b.dataset.open!)),
  );
}

function parseRef(ref: string): { providerId: number; streamId: number } {
  const [p, s] = ref.split(":");
  return { providerId: Number(p), streamId: Number(s) };
}

async function onFav(ref: string): Promise<void> {
  const { providerId, streamId } = parseRef(ref);
  const ch = channels.find((c) => c.provider_id === providerId && c.stream_id === streamId);
  const next = !(ch?.favorite ?? false);
  await api.setFavorite(providerId, streamId, next);
  await refreshLists();
}

async function onOpen(ref: string): Promise<void> {
  const { providerId, streamId } = parseRef(ref);
  const ch =
    channels.find((c) => c.provider_id === providerId && c.stream_id === streamId) ??
    recent.find((c) => c.provider_id === providerId && c.stream_id === streamId);
  await openPlayer({ providerId, streamId, name: ch?.name ?? "Live" });
  await api.recordRecent(providerId, streamId);
  await api.setSetting("last_channel", ref);
  await refreshLists();
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
