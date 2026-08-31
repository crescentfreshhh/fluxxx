// Providers view: add / enable / disable / delete Xtream providers, test the
// connection, and sync the catalog. Selecting "Curate" mounts the curation
// wizard (curator.ts) for that provider.
import { api, type Provider } from "../api";
import { renderCurator } from "./curator";

let root: HTMLElement;
let providers: Provider[] = [];
let selectedId: number | null = null;

export async function renderProviders(container: HTMLElement): Promise<void> {
  root = container;
  await refreshProviders();
}

async function refreshProviders(): Promise<void> {
  providers = await api.listProviders();
  if (selectedId && !providers.some((p) => p.id === selectedId)) selectedId = null;
  draw();
}

function draw(): void {
  root.innerHTML = `
    <div class="providers">
      <section class="panel">
        <h3>Your providers</h3>
        <div class="provider-list">${providers.map(providerCard).join("") || emptyState()}</div>
      </section>

      <section class="panel">
        <h3>Add a provider</h3>
        ${addForm()}
      </section>

      ${selectedId ? `<section class="panel curation" id="curator-host"></section>` : ""}
    </div>
  `;
  wire();

  if (selectedId) {
    const hostEl = root.querySelector<HTMLElement>("#curator-host");
    const p = providers.find((x) => x.id === selectedId);
    if (hostEl && p) void renderCurator(hostEl, p.id, p.name);
  }
}

function emptyState(): string {
  return `<p class="muted">No providers yet. Add one below to start pulling channels.</p>`;
}

function providerCard(p: Provider): string {
  const synced = p.last_synced_at ? relative(p.last_synced_at) : "never synced";
  return `
    <div class="provider-card ${p.id === selectedId ? "is-selected" : ""}" data-id="${p.id}">
      <div class="provider-main">
        <label class="switch" title="Enable / disable (disabled = fully excluded)">
          <input type="checkbox" data-act="toggle" data-id="${p.id}" ${p.enabled ? "checked" : ""} />
          <span class="slider"></span>
        </label>
        <div class="provider-meta">
          <div class="provider-name">${esc(p.name)}</div>
          <div class="provider-sub muted">${esc(p.host)}:${p.port} · ${esc(p.username)} · ${synced}</div>
        </div>
      </div>
      <div class="provider-actions">
        <button class="btn ${p.id === selectedId ? "btn-active" : ""}" data-act="curate" data-id="${p.id}">Curate</button>
        <button class="btn" data-act="sync" data-id="${p.id}" ${p.enabled ? "" : "disabled"}>Sync</button>
        <button class="btn btn-danger" data-act="delete" data-id="${p.id}">Delete</button>
      </div>
    </div>`;
}

function addForm(): string {
  return `
    <form class="add-form" data-form="add">
      <div class="row">
        <label>Name<input name="name" placeholder="My IPTV" required /></label>
      </div>
      <div class="row">
        <label class="grow">Host<input name="host" placeholder="http://example.com" required /></label>
        <label class="port">Port<input name="port" type="number" value="80" min="1" max="65535" required /></label>
      </div>
      <div class="row">
        <label class="grow">Username<input name="username" autocomplete="off" required /></label>
        <label class="grow">Password<input name="password" type="password" autocomplete="off" required /></label>
      </div>
      <div class="row actions">
        <button type="button" class="btn" data-act="test">Test connection</button>
        <button type="submit" class="btn btn-primary">Add provider</button>
        <span class="form-status" data-el="status"></span>
      </div>
    </form>`;
}

// --- events ------------------------------------------------------------------

function wire(): void {
  root.querySelectorAll<HTMLElement>(".providers > .panel [data-act]").forEach((elm) => {
    const act = elm.dataset.act!;
    if (act === "toggle") {
      elm.addEventListener("change", () =>
        void onToggle(Number(elm.dataset.id), (elm as HTMLInputElement).checked),
      );
    } else if (act) {
      elm.addEventListener("click", () => {
        const id = Number(elm.dataset.id);
        if (act === "sync") void onSync(id);
        else if (act === "delete") void onDelete(id);
        else if (act === "curate") void onCurate(id);
        else if (act === "test") void onTest();
      });
    }
  });

  const form = root.querySelector<HTMLFormElement>('[data-form="add"]');
  form?.addEventListener("submit", (e) => {
    e.preventDefault();
    void onAdd(form);
  });
}

function readForm(form: HTMLFormElement) {
  const data = new FormData(form);
  return {
    name: String(data.get("name") ?? "").trim(),
    host: String(data.get("host") ?? "").trim(),
    port: Number(data.get("port") ?? 80),
    username: String(data.get("username") ?? "").trim(),
    password: String(data.get("password") ?? ""),
  };
}

function setStatus(msg: string, kind: "ok" | "err" | "busy"): void {
  const el = root.querySelector<HTMLElement>('[data-el="status"]');
  if (el) {
    el.textContent = msg;
    el.className = `form-status ${kind}`;
  }
}

async function onTest(): Promise<void> {
  const form = root.querySelector<HTMLFormElement>('[data-form="add"]');
  if (!form) return;
  const input = readForm(form);
  if (!input.host || !input.username) {
    setStatus("Fill host, username and password first", "err");
    return;
  }
  setStatus("Testing…", "busy");
  try {
    const res = await api.testConnection(input);
    setStatus(res.message, res.ok ? "ok" : "err");
  } catch (e) {
    setStatus(String(e), "err");
  }
}

async function onAdd(form: HTMLFormElement): Promise<void> {
  const input = readForm(form);
  if (!input.name || !input.host || !input.username || !input.password) {
    setStatus("All fields are required", "err");
    return;
  }
  setStatus("Adding…", "busy");
  try {
    await api.addProvider(input);
    await refreshProviders();
  } catch (e) {
    setStatus(String(e), "err");
  }
}

async function onToggle(id: number, enabled: boolean): Promise<void> {
  try {
    await api.setProviderEnabled(id, enabled);
  } catch (e) {
    console.error(e);
  }
  await refreshProviders();
}

async function onDelete(id: number): Promise<void> {
  const p = providers.find((x) => x.id === id);
  if (!confirm(`Delete provider "${p?.name}"? Cached channels are removed (credentials too).`)) return;
  await api.deleteProvider(id);
  if (selectedId === id) selectedId = null;
  await refreshProviders();
}

async function onSync(id: number): Promise<void> {
  const wasFirstSync = !providers.find((x) => x.id === id)?.last_synced_at;
  const btn = root.querySelector<HTMLButtonElement>(`[data-act="sync"][data-id="${id}"]`);
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Syncing…";
  }
  try {
    const res = await api.syncProvider(id);
    // Open the curator automatically after the first sync so a huge dump gets
    // curated before it floods anything.
    if (wasFirstSync) selectedId = id;
    await refreshProviders();
    flash(`Synced: ${res.categories} categories, ${res.channels.toLocaleString()} channels`);
  } catch (e) {
    await refreshProviders();
    flash(`Sync failed: ${e}`, true);
  }
}

async function onCurate(id: number): Promise<void> {
  selectedId = selectedId === id ? null : id;
  draw();
}

// --- helpers -----------------------------------------------------------------

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

function relative(ts: number): string {
  const secs = Math.max(0, Math.floor(Date.now() / 1000) - ts);
  if (secs < 60) return "just now";
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
