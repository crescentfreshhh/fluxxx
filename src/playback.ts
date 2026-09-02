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
}

export const DEFAULT_VLC = "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe";
export const DEFAULT_VLC_ARGS = "--qt-minimal-view";
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
  externalContainer: "ts",
  hdrKeywords: DEFAULT_HDR_KEYWORDS,
  hdrVlcArgs: DEFAULT_HDR_VLC_ARGS,
};

export async function loadPlaybackConfig(): Promise<void> {
  const [backend, vlcPath, vlcArgs, externalCommand, container, hdrKeywords, hdrVlcArgs] =
    await Promise.all([
      api.getSetting("player_backend"),
      api.getSetting("vlc_path"),
      api.getSetting("vlc_args"),
      api.getSetting("external_command"),
      api.getSetting("external_container"),
      api.getSetting("hdr_keywords"),
      api.getSetting("hdr_vlc_args"),
    ]);
  if (backend === "webview" || backend === "vlc" || backend === "external") config.backend = backend;
  if (vlcPath) config.vlcPath = vlcPath;
  if (vlcArgs !== null && vlcArgs !== undefined) config.vlcArgs = vlcArgs;
  if (externalCommand) config.externalCommand = externalCommand;
  if (container === "ts" || container === "m3u8") config.externalContainer = container;
  if (hdrKeywords !== null && hdrKeywords !== undefined) config.hdrKeywords = hdrKeywords;
  if (hdrVlcArgs !== null && hdrVlcArgs !== undefined) config.hdrVlcArgs = hdrVlcArgs;
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
  await Promise.all(writes);
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
  }
  await api.launchExternal(command, [...extraArgs, url]);
}
