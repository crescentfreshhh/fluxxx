// Playback dispatch. The user picks a backend in Settings:
//   - "webview": lean in-app hls.js player (H.264 HLS, no external deps)
//   - "vlc":     launch VLC with the stream (full codec support, popout window)
//   - "external": launch any configured player (mpv, PotPlayer, …)
// Config is persisted in the backend settings table and cached here.
import { api } from "./api";
import { isHdr } from "./groups";
import { openPlayer } from "./views/player";

export type Backend = "webview" | "vlc" | "external";

export interface PlaybackConfig {
  backend: Backend;
  /** Path to VLC (defaults to the standard Windows install location). */
  vlcPath: string;
  /** Extra VLC command-line args (e.g. minimal interface). */
  vlcArgs: string;
  /** Full command/path for the "external" backend. */
  externalCommand: string;
  /** Container handed to external players (VLC/mpv handle raw TS best). */
  externalContainer: "ts" | "m3u8";
  /** Comma-separated keywords that mark a channel as HDR (name/category match). */
  hdrKeywords: string;
  /** Extra VLC args applied ONLY to HDR-matched channels (brighten). */
  hdrVlcArgs: string;
  /** HTTP User-Agent VLC sends when fetching the stream (--http-user-agent).
   * Empty = send VLC's own default (today's behaviour). */
  httpUserAgent: string;
}

export const DEFAULT_VLC = "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe";
// --one-instance: reuse one VLC window when surfing (avoids stacking provider
// connections → max-connection drops). Caching absorbs jitter; --http-reconnect
// lets VLC transparently re-establish a dropped HTTP source without us relaunching.
export const DEFAULT_VLC_ARGS =
  "--one-instance --qt-minimal-view --network-caching=5000 --live-caching=5000 --http-reconnect";
// Minimum network-caching (ms) we enforce for live streams: too-low caching is a
// common cause of streams stalling/dropping after a minute or two.
export const MIN_LIVE_NET_CACHING = 3000;
// Default User-Agent: empty preserves VLC's built-in UA (unchanged behaviour).
export const DEFAULT_HTTP_USER_AGENT = "";
// Previous defaults — any still-saved copy is auto-upgraded to DEFAULT_VLC_ARGS so
// existing installs get the reliability fix without clobbering genuinely custom args.
const PREV_DEFAULT_VLC_ARGS = [
  "--qt-minimal-view",
  "--one-instance --qt-minimal-view --network-caching=5000 --live-caching=5000",
];
export const DEFAULT_HDR_KEYWORDS = "HDR, HDR10, DV, Dolby Vision";
export const DEFAULT_HDR_VLC_ARGS =
  "--video-filter=adjust --brightness=1.2 --gamma=1.4 --contrast=1.05 --saturation=1.35";

// Default to VLC: the in-app webview player can't decode many IPTV streams
// (H.265/raw TS). A saved choice still overrides this.
let config: PlaybackConfig = {
  backend: "vlc",
  vlcPath: DEFAULT_VLC,
  vlcArgs: DEFAULT_VLC_ARGS,
  externalCommand: "",
  externalContainer: "m3u8",
  hdrKeywords: DEFAULT_HDR_KEYWORDS,
  hdrVlcArgs: DEFAULT_HDR_VLC_ARGS,
  httpUserAgent: DEFAULT_HTTP_USER_AGENT,
};

export async function loadPlaybackConfig(): Promise<void> {
  const [backend, vlcPath, vlcArgs, externalCommand, container, hdrKeywords, hdrVlcArgs, httpUserAgent] =
    await Promise.all([
      api.getSetting("player_backend"),
      api.getSetting("vlc_path"),
      api.getSetting("vlc_args"),
      api.getSetting("external_command"),
      api.getSetting("external_container"),
      api.getSetting("hdr_keywords"),
      api.getSetting("hdr_vlc_args"),
      api.getSetting("http_user_agent"),
    ]);
  if (backend === "webview" || backend === "vlc" || backend === "external") config.backend = backend;
  if (vlcPath) config.vlcPath = vlcPath;
  // Auto-upgrade any previous default to the new one; keep genuinely custom args.
  if (vlcArgs && !PREV_DEFAULT_VLC_ARGS.includes(vlcArgs.trim())) config.vlcArgs = vlcArgs;
  else config.vlcArgs = DEFAULT_VLC_ARGS;
  if (externalCommand) config.externalCommand = externalCommand;
  if (container === "ts" || container === "m3u8") config.externalContainer = container;
  if (hdrKeywords !== null && hdrKeywords !== undefined) config.hdrKeywords = hdrKeywords;
  if (hdrVlcArgs !== null && hdrVlcArgs !== undefined) config.hdrVlcArgs = hdrVlcArgs;
  if (httpUserAgent !== null && httpUserAgent !== undefined) config.httpUserAgent = httpUserAgent;
}

/** True if the given channel text matches the configured HDR keywords. */
export function matchesHdr(text: string): boolean {
  return isHdr(text, config.hdrKeywords.split(","));
}

/** Split a VLC args string into argv tokens (simple whitespace split). */
export function parseArgs(s: string): string[] {
  return s.split(/\s+/).filter(Boolean);
}

export function getPlaybackConfig(): PlaybackConfig {
  return { ...config };
}

export async function setPlaybackConfig(next: Partial<PlaybackConfig>): Promise<void> {
  config = { ...config, ...next };
  const writes: Promise<unknown>[] = [];
  if (next.backend) writes.push(api.setSetting("player_backend", next.backend));
  if (next.vlcPath !== undefined) writes.push(api.setSetting("vlc_path", next.vlcPath));
  if (next.vlcArgs !== undefined) writes.push(api.setSetting("vlc_args", next.vlcArgs));
  if (next.externalCommand !== undefined)
    writes.push(api.setSetting("external_command", next.externalCommand));
  if (next.externalContainer) writes.push(api.setSetting("external_container", next.externalContainer));
  if (next.hdrKeywords !== undefined) writes.push(api.setSetting("hdr_keywords", next.hdrKeywords));
  if (next.hdrVlcArgs !== undefined) writes.push(api.setSetting("hdr_vlc_args", next.hdrVlcArgs));
  if (next.httpUserAgent !== undefined) writes.push(api.setSetting("http_user_agent", next.httpUserAgent));
  await Promise.all(writes);
}

/** Hide the user/password path segments of an Xtream stream URL for logging. */
function maskUrl(url: string): string {
  return url.replace(/\/live\/[^/]+\/[^/]+\//, "/live/***/***/");
}

/** Ensure the live-reliability VLC options are present without clobbering the
 * user's own args: guarantee --http-reconnect, and raise both --network-caching
 * and --live-caching to at least MIN_LIVE_NET_CACHING ms (VLC's own defaults of
 * ~1000 ms are a frequent cause of streams dropping after a minute or two). */
export function ensureLiveVlcOptions(tokens: string[]): string[] {
  let out = [...tokens];
  if (!out.some((t) => t === "--http-reconnect" || t.startsWith("--http-reconnect="))) {
    out.push("--http-reconnect");
  }
  out = ensureMinCaching(out, "--network-caching");
  out = ensureMinCaching(out, "--live-caching");
  return out;
}

/** Ensure a `<flag>=<ms>` caching option is present and at least the live
 * minimum; adds it if missing, raises it if set lower, leaves higher values. */
function ensureMinCaching(tokens: string[], flag: string): string[] {
  const out = [...tokens];
  const idx = out.findIndex((t) => t.startsWith(flag));
  if (idx === -1) {
    out.push(`${flag}=${MIN_LIVE_NET_CACHING}`);
    return out;
  }
  const m = new RegExp(`${flag}=(\\d+)`).exec(out[idx]);
  const val = m ? parseInt(m[1], 10) : NaN;
  if (!Number.isFinite(val) || val < MIN_LIVE_NET_CACHING) {
    out[idx] = `${flag}=${MIN_LIVE_NET_CACHING}`;
  }
  return out;
}

// Previous stream URL handed to a backend, so we can log a switch (the nearest
// thing to a "close" for --one-instance VLC, which replaces the input in place).
let lastOpenedUrl = "";

/** Timestamped open/switch log so each stream handoff is visible in the console. */
function logOpen(backend: Backend, url: string, argv: string[]): void {
  const ts = new Date().toISOString();
  if (lastOpenedUrl && lastOpenedUrl !== url) {
    console.info(`[playback ${ts}] SWITCH from ${maskUrl(lastOpenedUrl)} (previous stream superseded)`);
  }
  const maskedArgv = argv.map((a) => (a.includes("://") ? maskUrl(a) : a));
  console.info(`[playback ${ts}] OPEN backend=${backend} url=${maskUrl(url)} argv=${JSON.stringify(maskedArgv)}`);
  lastOpenedUrl = url;
}

export interface PlayTarget {
  providerId: number;
  streamId: number;
  name: string;
  /** Whether this channel is HDR (caller computes via matchesHdr). */
  hdr?: boolean;
}

/** Play a channel via the configured backend. Throws if an external backend has
 * no command configured (the caller surfaces the error). */
export async function playChannel(t: PlayTarget): Promise<void> {
  if (config.backend === "webview") {
    await openPlayer(t);
    return;
  }
  const command = config.backend === "vlc" ? config.vlcPath || DEFAULT_VLC : config.externalCommand;
  if (!command) {
    throw new Error("No external player configured — set one in Settings.");
  }
  const url = await api.streamUrl(t.providerId, t.streamId, config.externalContainer);
  let extraArgs: string[] = [];
  if (config.backend === "vlc") {
    extraArgs = parseArgs(config.vlcArgs);
    if (t.hdr) extraArgs = [...extraArgs, ...parseArgs(config.hdrVlcArgs)];
    // Guarantee the live-reliability options regardless of custom args.
    extraArgs = ensureLiveVlcOptions(extraArgs);
    const ua = config.httpUserAgent.trim();
    if (ua) extraArgs.push(`--http-user-agent=${ua}`);
  }
  const argv = [...extraArgs, url];
  logOpen(config.backend, url, argv);
  await api.launchExternal(command, argv);
}
