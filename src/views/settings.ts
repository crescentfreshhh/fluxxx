// Settings: choose the playback backend (in-app hls.js / VLC / custom external)
// and control the Go-Live window (size presets, borderless, always-on-top).
import {
  getPlaybackConfig,
  setPlaybackConfig,
  DEFAULT_VLC,
  type Backend,
} from "../playback";
import { setPresetSize, setBorderless, setAlwaysOnTop, isBorderless, isAlwaysOnTop } from "../window";

let root: HTMLElement;

export async function renderSettings(container: HTMLElement): Promise<void> {
  root = container;
  const borderless = await isBorderless();
  const onTop = await isAlwaysOnTop();
  draw(borderless, onTop);
}

function draw(borderless: boolean, onTop: boolean): void {
  const cfg = getPlaybackConfig();
  root.innerHTML = `
    <div class="settings">
      <section class="panel">
        <h3>Playback</h3>
        <p class="muted">Choose how channels play. VLC and external players handle codecs
          (H.265, raw TS) the in-app player can't.</p>

        <div class="backend-opts">
          ${backendOption("webview", "In-app player", "Lean hls.js player inside fluxxx. H.264 HLS, no external app.", cfg.backend)}
          ${backendOption("vlc", "VLC (popout)", "Launch VLC with the stream — full codec support, lightweight window.", cfg.backend)}
          ${backendOption("external", "Custom external", "Launch any player (mpv, PotPlayer, …) by path.", cfg.backend)}
        </div>

        ${cfg.backend === "vlc" ? vlcFields(cfg.vlcPath, cfg.externalContainer) : ""}
        ${cfg.backend === "external" ? externalFields(cfg.externalCommand, cfg.externalContainer) : ""}
      </section>

      <section class="panel">
        <h3>Go-Live window</h3>
        <p class="muted">Size fluxxx for Discord's Go-Live, drop the window chrome for a clean
          capture, and pin it on top. Audio is captured by Discord automatically.</p>

        <div class="golive-row">
          <span class="golive-label">Size</span>
          <button class="btn" data-size="1280x720">1280 × 720</button>
          <button class="btn" data-size="1920x1080">1920 × 1080</button>
          <button class="btn" data-size="1600x900">1600 × 900</button>
        </div>
        <label class="golive-toggle">
          <span class="switch"><input type="checkbox" data-win="borderless" ${borderless ? "checked" : ""}/><span class="slider"></span></span>
          <span>Borderless (clean capture surface)</span>
        </label>
        <label class="golive-toggle">
          <span class="switch"><input type="checkbox" data-win="ontop" ${onTop ? "checked" : ""}/><span class="slider"></span></span>
          <span>Always on top</span>
        </label>
      </section>
    </div>`;
  wire();
}

function backendOption(value: Backend, title: string, desc: string, current: Backend): string {
  return `
    <label class="backend-opt ${value === current ? "sel" : ""}">
      <input type="radio" name="backend" value="${value}" ${value === current ? "checked" : ""} />
      <div>
        <div class="backend-title">${title}</div>
        <div class="backend-desc muted">${desc}</div>
      </div>
    </label>`;
}

function vlcFields(path: string, container: string): string {
  return `
    <div class="field">
      <label>VLC path</label>
      <input class="cfg-input" data-cfg="vlcPath" value="${esc(path || DEFAULT_VLC)}" placeholder="${esc(DEFAULT_VLC)}" />
    </div>
    ${containerField(container)}`;
}

function externalFields(command: string, container: string): string {
  return `
    <div class="field">
      <label>Player command / path</label>
      <input class="cfg-input" data-cfg="externalCommand" value="${esc(command)}" placeholder="C:\\path\\to\\mpv.exe" />
    </div>
    ${containerField(container)}`;
}

function containerField(container: string): string {
  return `
    <div class="field">
      <label>Stream format for external player</label>
      <select class="cfg-input" data-cfg="externalContainer">
        <option value="ts" ${container === "ts" ? "selected" : ""}>Raw TS (.ts) — best for VLC/mpv</option>
        <option value="m3u8" ${container === "m3u8" ? "selected" : ""}>HLS (.m3u8)</option>
      </select>
    </div>`;
}

function wire(): void {
  root.querySelectorAll<HTMLInputElement>('input[name="backend"]').forEach((r) =>
    r.addEventListener("change", async () => {
      await setPlaybackConfig({ backend: r.value as Backend });
      const b = await isBorderless();
      const t = await isAlwaysOnTop();
      draw(b, t);
    }),
  );

  root.querySelectorAll<HTMLElement>("[data-cfg]").forEach((el) => {
    const key = el.dataset.cfg!;
    const handler = () => {
      const value = (el as HTMLInputElement | HTMLSelectElement).value;
      void setPlaybackConfig({ [key]: value } as never);
    };
    el.addEventListener("change", handler);
  });

  root.querySelectorAll<HTMLButtonElement>("[data-size]").forEach((b) =>
    b.addEventListener("click", () => {
      const [w, h] = b.dataset.size!.split("x").map(Number);
      void setPresetSize(w, h);
    }),
  );

  root.querySelector<HTMLInputElement>('[data-win="borderless"]')?.addEventListener("change", (e) => {
    void setBorderless((e.target as HTMLInputElement).checked);
  });
  root.querySelector<HTMLInputElement>('[data-win="ontop"]')?.addEventListener("change", (e) => {
    void setAlwaysOnTop((e.target as HTMLInputElement).checked);
  });
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
