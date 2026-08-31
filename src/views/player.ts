// In-webview live player. Xtream exposes an HLS (.m3u8) endpoint which WebView2
// (Chromium) plays via hls.js/MSE — no native player or DLL needed, and the
// window is a clean Go-Live capture surface. H.265/raw-TS-only streams may not
// decode here; that would need a future native (libmpv) backend.
//
// hls.js (~190KB gzip) is lazy-loaded on first play so the app stays lean.
import { api } from "../api";

// Loaded on demand; `any` avoids pulling hls.js types into the eager bundle.
/* eslint-disable @typescript-eslint/no-explicit-any */
let HlsMod: any = null;
let overlay: HTMLElement | null = null;
let hls: any = null;
let keyHandler: ((e: KeyboardEvent) => void) | null = null;

async function getHls(): Promise<any> {
  if (!HlsMod) HlsMod = (await import("hls.js")).default;
  return HlsMod;
}

export interface PlayOpts {
  providerId: number;
  streamId: number;
  name: string;
}

export async function openPlayer(opts: PlayOpts): Promise<void> {
  close(); // tear down any previous session

  overlay = document.createElement("div");
  overlay.className = "player-overlay";
  overlay.innerHTML = `
    <div class="player-bar">
      <div class="player-title">${esc(opts.name)}</div>
      <div class="player-actions">
        <span class="player-status" data-el="status">Connecting…</span>
        <button class="player-close" title="Close (Esc)">✕</button>
      </div>
    </div>
    <video class="player-video" autoplay playsinline></video>
    <div class="player-message" data-el="msg" hidden></div>`;
  document.body.appendChild(overlay);

  const video = overlay.querySelector<HTMLVideoElement>(".player-video")!;
  overlay.querySelector<HTMLButtonElement>(".player-close")?.addEventListener("click", () => close());
  keyHandler = (e) => {
    if (e.key === "Escape") close();
  };
  window.addEventListener("keydown", keyHandler);

  try {
    const url = await api.streamUrl(opts.providerId, opts.streamId, "m3u8");
    await attach(video, url);
  } catch (e) {
    showMessage(`Could not start stream: ${e}`);
  }
}

async function attach(video: HTMLVideoElement, url: string): Promise<void> {
  const Hls = await getHls();
  if (!overlay) return; // closed while loading

  if (Hls.isSupported()) {
    hls = new Hls({ enableWorker: true, lowLatencyMode: true, backBufferLength: 30 });
    hls.attachMedia(video);
    hls.on(Hls.Events.MEDIA_ATTACHED, () => hls.loadSource(url));
    hls.on(Hls.Events.MANIFEST_PARSED, () => {
      setStatus("Live");
      void video.play().catch(() => {});
    });
    hls.on(Hls.Events.ERROR, (_evt: unknown, data: any) => {
      if (!data.fatal) return;
      if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
        setStatus("Reconnecting…");
        hls.startLoad();
      } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
        setStatus("Recovering…");
        hls.recoverMediaError();
      } else {
        showMessage(
          "This stream couldn't be played in-app. It may use a codec (e.g. H.265) " +
            "or format the webview can't decode.",
        );
      }
    });
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = url;
    setStatus("Live");
    void video.play().catch(() => {});
  } else {
    showMessage("HLS playback is not supported in this webview.");
  }
}

function setStatus(text: string): void {
  const el = overlay?.querySelector<HTMLElement>('[data-el="status"]');
  if (el) el.textContent = text;
}

function showMessage(text: string): void {
  const el = overlay?.querySelector<HTMLElement>('[data-el="msg"]');
  if (el) {
    el.textContent = text;
    el.hidden = false;
  }
  setStatus("Error");
}

export function close(): void {
  if (hls) {
    hls.destroy();
    hls = null;
  }
  if (keyHandler) {
    window.removeEventListener("keydown", keyHandler);
    keyHandler = null;
  }
  if (overlay) {
    overlay.remove();
    overlay = null;
  }
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
