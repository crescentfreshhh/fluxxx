import { api } from "./api";
import { renderProviders } from "./views/providers";
import { renderLive } from "./views/live";
import { renderGuide } from "./views/guide";
import "./styles.css";

type ViewFn = (root: HTMLElement) => void | Promise<void>;

const views: Record<string, ViewFn> = {
  live: (root) => renderLive(root),
  guide: (root) => renderGuide(root),
  providers: (root) => renderProviders(root),
  settings: placeholder("Settings", "Settings will arrive alongside playback."),
};

function placeholder(title: string, body: string): ViewFn {
  return (root) => {
    root.innerHTML = `
      <div class="placeholder">
        <div class="placeholder-icon">▶</div>
        <h2>${title}</h2>
        <p class="muted">${body}</p>
      </div>`;
  };
}

async function activate(view: string): Promise<void> {
  const root = document.getElementById("view-root");
  const title = document.getElementById("view-title");
  if (!root) return;

  document.querySelectorAll<HTMLButtonElement>(".nav-item").forEach((b) => {
    b.classList.toggle("is-active", b.dataset.view === view);
    if (b.dataset.view === view && title) title.textContent = b.textContent ?? "";
  });

  const fn = views[view] ?? views.providers;
  root.innerHTML = "";
  await fn(root);
}

function wireNav(): void {
  document.querySelectorAll<HTMLButtonElement>(".nav-item").forEach((btn) => {
    btn.addEventListener("click", () => void activate(btn.dataset.view ?? "providers"));
  });
}

async function showVersion(): Promise<void> {
  const badge = document.getElementById("version-badge");
  if (!badge) return;
  try {
    const info = await api.appInfo();
    badge.textContent = `${info.name} v${info.version}`;
    badge.classList.add("ok");
  } catch (err) {
    badge.textContent = "backend offline";
    badge.classList.add("err");
    console.error(err);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  void showVersion();
  wireNav();
  void activate("live");
});
