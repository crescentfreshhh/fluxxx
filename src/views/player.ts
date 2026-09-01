// In-webview live player. Xtream exposes an HLS (.m3u8) endpoint which WebView2
// (Chromium) plays via hls.js/MSE. Many IPTV streams are H.265/HEVC or raw TS
// that Chromium can't decode — for those the built-in Info panel surfaces the
// codec/error and the "Open in VLC" button hands the same channel to VLC.
//
// hls.js (~190KB gzip) is lazy-loaded on first play so the app stays lean.
import { api } from "../api";

/* eslint-disable @typescript-eslint/no-explicit-any */
const DEFAULT_VLC = "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe";
const MAX_NET_RETRIES = 3;
const LOG_MAX = 16;

let HlsMod: any = null;
let overlay: HTMLElement | null = null;
let hls: any = null;
let Hls: any = null;
let keyHandler: ((e: KeyboardEvent) => void) | null = null;
let statsTimer: number | undefined;

let opts: PlayOpts | null = null;
let currentUrl = "";
let netRetries = 0;
let mediaRecovered = false;
let fragCount = 0;
let lastError = "—";
let log: string[] = [];

async function getHls(): Promise<any> {
  if (!HlsMod) HlsMod = (await import("hls.js")).default;
  return HlsMod;
}

export interface PlayOpts {
  providerId: number;
  streamId: number;
  name: string;
}

export async function openPlayer(o: PlayOpts): Promise<void> {
  close();
  opts = o;
  netRetries = 0;
  mediaRecovered = false;
  fragCount = 0;
  lastError = "—";
  log = [];

  overlay = document.createElement("div");
  overlay.className = "player-overlay";
  overlay.innerHTML = `
    <div class="player-bar">
      <div class="player-title">${esc(o.name)}</div>
      <div class="player-actions">
        <span class="player-status" data-el="status">Connecting…</span>
        <button class="pbtn" data-act="vlc" title="Open this channel in VLC">Open in VLC</button>
        <button class="pbtn" data-act="copy" title="Copy stream URL">Copy URL</button>
        <button class="pbtn" data-act="info" title="Diagnostics (i)">Info</button>
        <button class="pbtn player-close" data-act="close" title="Close (Esc)">✕</button>
      </div>
    </div>
    <video class="player-video" autoplay playsinline controls></video>
    <div class="player-diag" data-el="diag" hidden><pre data-el="diagpre"></pre></div>
    <div class="player-message" data-el="msg" hidden></div>`;
  document.body.appendChild(overlay);

  const video = overlay.querySelector<HTMLVideoElement>(".player-video")!;
  overlay.querySelectorAll<HTMLButtonElement>("[data-act]").forEach((b) =>
    b.addEventListener("click", () => onAction(b.dataset.act!)),
  );
  keyHandler = (e) => {
    if (e.key === "Escape") close();
    else if (e.key.toLowerCase() === "i") toggleDiag();
  };
  window.addEventListener("keydown", keyHandler);

  statsTimer = window.setInterval(updateDiag, 1000);

  try {
    currentUrl = await api.streamUrl(o.providerId, o.streamId, "m3u8");
    addLog(`stream ${maskUrl(currentUrl)}`);
    await attach(video, currentUrl);
  } catch (e) {
    showMessage(`Could not start stream: ${e}`);
  }
}

async function attach(video: HTMLVideoElement, url: string): Promise<void> {
  Hls = await getHls();
  if (!overlay) return;

  if (Hls.isSupported()) {
    hls = new Hls({ enableWorker: true, lowLatencyMode: true, backBufferLength: 30 });
    hls.attachMedia(video);
    hls.on(Hls.Events.MEDIA_ATTACHED, () => hls.loadSource(url));

    hls.on(Hls.Events.MANIFEST_PARSED, (_e: unknown, data: any) => {
      setStatus("Live");
      addLog(`manifest parsed — ${data?.levels?.length ?? "?"} level(s)`);
      const hevc = (hls.levels ?? []).some((l: any) => isHevc(l.videoCodec) || isHevc(l.attrs?.CODECS));
      if (hevc) {
        addLog("HEVC/H.265 detected in levels");
        showMessage(
          "This stream looks like H.265 (HEVC), which the in-app player can't decode. " +
            "Use “Open in VLC”.",
          true,
        );
      }
      void video.play().catch(() => {});
    });

    hls.on(Hls.Events.LEVEL_SWITCHED, (_e: unknown, data: any) => {
      const l = hls.levels?.[data.level];
      if (l) addLog(`level → ${l.width}x${l.height} @ ${Math.round((l.bitrate || 0) / 1000)}kbps`);
    });

    hls.on(Hls.Events.FRAG_BUFFERED, () => {
      fragCount += 1;
      if (fragCount === 1) {
        setStatus("Live");
        addLog("first segment buffered — playing");
      }
    });

    hls.on(Hls.Events.ERROR, (_e: unknown, data: any) => {
      lastError = `${data.type} / ${data.details}${
        data.response?.code ? ` (HTTP ${data.response.code})` : ""
      }`;
      if (!data.fatal) return;
      addLog(`FATAL ${lastError}`);

      if (isCodecError(data.details)) {
        showMessage(
          "The webview can't decode this stream's codec (likely H.265). Use “Open in VLC”.",
          true,
        );
        return;
      }
      if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
        if (netRetries < MAX_NET_RETRIES) {
          netRetries += 1;
          setStatus(`Reconnecting… (${netRetries}/${MAX_NET_RETRIES})`);
          hls.startLoad();
        } else {
          showMessage(
            "Couldn't load the stream after several tries — the server may be unreachable, " +
              "the credentials/port may be off, or the format isn't HLS. Try “Open in VLC”.",
            true,
          );
        }
      } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
        if (!mediaRecovered) {
          mediaRecovered = true;
          setStatus("Recovering…");
          hls.recoverMediaError();
        } else {
          showMessage("Playback failed (media error). Try “Open in VLC”.", true);
        }
      } else {
        showMessage("This stream couldn't be played in-app. Try “Open in VLC”.", true);
      }
    });
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = url;
    setStatus("Live");
    void video.play().catch(() => {});
  } else {
    showMessage("HLS playback isn't supported in this webview.");
  }
}

// --- actions -----------------------------------------------------------------

function onAction(act: string): void {
  if (act === "close") close();
  else if (act === "info") toggleDiag();
  else if (act === "copy") void copyUrl();
  else if (act === "vlc") void openInVlc();
}

async function openInVlc(): Promise<void> {
  if (!opts) return;
  try {
    const vlc = (await api.getSetting("vlc_path")) || DEFAULT_VLC;
    const argsStr = (await api.getSetting("vlc_args")) ?? "--qt-minimal-view";
    const argv = argsStr.split(/\s+/).filter(Boolean);
    const tsUrl = await api.streamUrl(opts.providerId, opts.streamId, "ts");
    await api.launchExternal(vlc, [...argv, tsUrl]);
    toast("Opening in VLC…");
    // Hand off to VLC; the black in-app surface is no longer useful.
    setTimeout(() => close(), 600);
  } catch (e) {
    toast(`VLC launch failed: ${e}. Set the VLC path in Settings → Playback.`, true);
  }
}

async function copyUrl(): Promise<void> {
  try {
    await navigator.clipboard.writeText(currentUrl);
    toast("Stream URL copied");
  } catch {
    // Fallback for non-secure contexts.
    const ta = document.createElement("textarea");
    ta.value = currentUrl;
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand("copy");
      toast("Stream URL copied");
    } catch {
      toast("Couldn't copy URL", true);
    }
    ta.remove();
  }
}

function toggleDiag(): void {
  const d = overlay?.querySelector<HTMLElement>('[data-el="diag"]');
  if (d) {
    d.hidden = !d.hidden;
    if (!d.hidden) updateDiag();
  }
}

// --- diagnostics -------------------------------------------------------------

function updateDiag(): void {
  const pre = overlay?.querySelector<HTMLElement>('[data-el="diagpre"]');
  if (!pre) return;
  const video = overlay?.querySelector<HTMLVideoElement>(".player-video");

  let level = "—";
  if (hls && hls.levels && hls.levels.length) {
    const idx = hls.currentLevel >= 0 ? hls.currentLevel : hls.firstLevel ?? 0;
    const l = hls.levels[idx];
    if (l) {
      const codecs = [l.videoCodec, l.audioCodec].filter(Boolean).join(" / ") || "unknown";
      level = `${l.width || "?"}x${l.height || "?"} @ ${Math.round((l.bitrate || 0) / 1000)}kbps  [${codecs}]`;
    }
  }

  let buffered = "—";
  if (video && video.buffered.length) {
    const end = video.buffered.end(video.buffered.length - 1);
    buffered = `${Math.max(0, end - video.currentTime).toFixed(1)}s ahead`;
  }

  const statusEl = overlay?.querySelector<HTMLElement>('[data-el="status"]');
  const lines = [
    `Backend    : In-app (hls.js)`,
    `Channel    : ${opts?.name ?? "—"}`,
    `URL        : ${maskUrl(currentUrl)}`,
    `Status     : ${statusEl?.textContent ?? "—"}`,
    `Level      : ${level}`,
    `Buffered   : ${buffered}`,
    `Segments   : ${fragCount}`,
    `Retries    : ${netRetries}/${MAX_NET_RETRIES}`,
    `Last error : ${lastError}`,
    `———————————————— events ————————————————`,
    ...log,
  ];
  pre.textContent = lines.join("\n");
}

function addLog(line: string): void {
  const t = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  log.push(`${t}  ${line}`);
  if (log.length > LOG_MAX) log = log.slice(-LOG_MAX);
  const d = overlay?.querySelector<HTMLElement>('[data-el="diag"]');
  if (d && !d.hidden) updateDiag();
}

function setStatus(text: string): void {
  const el = overlay?.querySelector<HTMLElement>('[data-el="status"]');
  if (el) el.textContent = text;
}

function showMessage(text: string, keepVideo = false): void {
  const el = overlay?.querySelector<HTMLElement>('[data-el="msg"]');
  if (el) {
    el.innerHTML = `<div class="pm-box">${esc(text)}</div>`;
    el.hidden = false;
  }
  if (!keepVideo) setStatus("Error");
  else setStatus("Can't decode");
}

// --- helpers -----------------------------------------------------------------

function isHevc(codec?: string): boolean {
  return !!codec && /(^|[,.\s])(hvc1|hev1|hvc|hevc|h265|h\.265)/i.test(codec);
}

function isCodecError(details?: string): boolean {
  return (
    details === "bufferAddCodecError" ||
    details === "bufferIncompatibleCodecsError" ||
    details === "manifestIncompatibleCodecsError"
  );
}

/** Hide the user/password path segments in an Xtream URL for display. */
function maskUrl(url: string): string {
  return url.replace(/\/live\/[^/]+\/[^/]+\//, "/live/***/***/");
}

export function close(): void {
  if (statsTimer) {
    window.clearInterval(statsTimer);
    statsTimer = undefined;
  }
  if (hls) {
    try {
      hls.destroy();
    } catch {
      /* ignore */
    }
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

function toast(msg: string, err = false): void {
  const t = document.createElement("div");
  t.className = `toast ${err ? "err" : "ok"}`;
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.classList.add("show"), 10);
  setTimeout(() => {
    t.classList.remove("show");
    setTimeout(() => t.remove(), 300);
  }, 3600);
}

function esc(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
