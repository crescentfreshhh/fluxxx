// Playback dispatch. The user picks a backend in Settings:
//   - "webview": lean in-app hls.js player (H.264 HLS, no external deps)
//   - "vlc":     launch VLC with the stream (full codec support, popout window)
//   - "external": launch any configured player (mpv, PotPlayer, …)
// Config is persisted in the backend settings table and cached here.
import { api } from "./api";
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
}

export const DEFAULT_VLC = "C:\\Program Files\\VideoLAN\\VLC\\vlc.exe";
export const DEFAULT_VLC_ARGS = "--qt-minimal-view";

// Default to VLC: the in-app webview player can't decode many IPTV streams
// (H.265/raw TS). A saved choice still overrides this.
let config: PlaybackConfig = {
  backend: "vlc",
  vlcPath: DEFAULT_VLC,
  vlcArgs: DEFAULT_VLC_ARGS,
  externalCommand: "",
  externalContainer: "ts",
};

export async function loadPlaybackConfig(): Promise<void> {
  const [backend, vlcPath, vlcArgs, externalCommand, container] = await Promise.all([
    api.getSetting("player_backend"),
    api.getSetting("vlc_path"),
    api.getSetting("vlc_args"),
    api.getSetting("external_command"),
    api.getSetting("external_container"),
  ]);
  if (backend === "webview" || backend === "vlc" || backend === "external") config.backend = backend;
  if (vlcPath) config.vlcPath = vlcPath;
  if (vlcArgs !== null && vlcArgs !== undefined) config.vlcArgs = vlcArgs;
  if (externalCommand) config.externalCommand = externalCommand;
  if (container === "ts" || container === "m3u8") config.externalContainer = container;
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
  await Promise.all(writes);
}

export interface PlayTarget {
  providerId: number;
  streamId: number;
  name: string;
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
  const extraArgs = config.backend === "vlc" ? parseArgs(config.vlcArgs) : [];
  await api.launchExternal(command, [...extraArgs, url]);
}
