// Go-Live window helpers. Discord's Go-Live captures a chosen window, so these
// let the user size fluxxx to a standard capture resolution, drop the window
// chrome for a clean surface, and pin it on top. Preferences persist so the
// window comes back the way they left it.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { api } from "./api";

export async function setPresetSize(w: number, h: number): Promise<void> {
  const win = getCurrentWindow();
  await win.setSize(new LogicalSize(w, h));
  await win.center();
}

export async function setBorderless(on: boolean): Promise<void> {
  await getCurrentWindow().setDecorations(!on);
  await api.setSetting("win_borderless", on ? "1" : "0");
}

export async function setAlwaysOnTop(on: boolean): Promise<void> {
  await getCurrentWindow().setAlwaysOnTop(on);
  await api.setSetting("win_always_on_top", on ? "1" : "0");
}

export async function isBorderless(): Promise<boolean> {
  return (await api.getSetting("win_borderless")) === "1";
}

export async function isAlwaysOnTop(): Promise<boolean> {
  return (await api.getSetting("win_always_on_top")) === "1";
}

/** Reapply saved window preferences at startup. */
export async function applySavedWindowPrefs(): Promise<void> {
  try {
    if (await isBorderless()) await getCurrentWindow().setDecorations(false);
    if (await isAlwaysOnTop()) await getCurrentWindow().setAlwaysOnTop(true);
  } catch (e) {
    console.error("failed to apply window prefs", e);
  }
}
