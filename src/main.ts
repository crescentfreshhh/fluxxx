import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

interface AppInfo {
  name: string;
  version: string;
}

interface Country {
  code: string;
  name: string;
}

async function showVersion(): Promise<void> {
  const badge = document.getElementById("version-badge");
  if (!badge) return;
  try {
    const info = await invoke<AppInfo>("app_info");
    badge.textContent = `${info.name} v${info.version}`;
    badge.classList.add("ok");
  } catch (err) {
    badge.textContent = "backend offline";
    badge.classList.add("err");
    console.error(err);
  }
}

function wireNav(): void {
  const items = document.querySelectorAll<HTMLButtonElement>(".nav-item");
  const title = document.getElementById("view-title");
  items.forEach((btn) => {
    btn.addEventListener("click", () => {
      items.forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
      if (title) title.textContent = btn.textContent ?? "";
    });
  });
}

function wireProbe(): void {
  const input = document.getElementById("probe-input") as HTMLInputElement | null;
  const result = document.getElementById("probe-result");
  if (!input || !result) return;

  let timer: number | undefined;
  const run = async () => {
    const name = input.value.trim();
    if (!name) {
      result.textContent = "";
      return;
    }
    try {
      const country = await invoke<Country | null>("infer_country", { name });
      result.textContent = country
        ? `→ ${country.name} (${country.code})`
        : "→ Other (no country inferred)";
      result.className = "probe-result " + (country ? "hit" : "miss");
    } catch (err) {
      result.textContent = "probe failed";
      result.className = "probe-result miss";
      console.error(err);
    }
  };

  input.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(run, 120);
  });
  run();
}

window.addEventListener("DOMContentLoaded", () => {
  void showVersion();
  wireNav();
  wireProbe();
});
