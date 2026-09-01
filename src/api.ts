// Typed wrappers around the Tauri command bridge. Keeps `invoke` string keys and
// payload shapes in one place so views stay clean.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
}

export interface Provider {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  enabled: boolean;
  created_at: number;
  last_synced_at: number | null;
}

export interface AddProviderInput {
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
}

export interface TestResult {
  ok: boolean;
  status: string | null;
  message: string;
}

export interface SyncResult {
  categories: number;
  channels: number;
}

export interface CountryGroup {
  code: string | null;
  name: string;
  channel_count: number;
  enabled_categories: number;
  total_categories: number;
  fully_enabled: boolean;
}

export interface Category {
  id: number;
  name: string;
  country_code: string | null;
  country_name: string | null;
  enabled: boolean;
  channel_count: number;
}

export interface CurationStats {
  total_categories: number;
  enabled_categories: number;
  total_channels: number;
  enabled_channels: number;
}

export interface Channel {
  id: number;
  provider_id: number;
  provider_name: string;
  stream_id: number;
  name: string;
  category_name: string | null;
  country_code: string | null;
  epg_channel_id: string | null;
  logo: string | null;
  num: number | null;
  favorite: boolean;
}

export interface ChannelQueryOpts {
  providerId?: number | null;
  categoryId?: number | null;
  search?: string | null;
  favoritesOnly?: boolean;
  limit?: number;
}

export interface ActiveCategory {
  id: number;
  name: string;
  country_code: string | null;
  provider_name: string;
  channel_count: number;
}

export interface EpgProgram {
  stream_id: number;
  start_utc: number;
  stop_utc: number;
  title: string;
  description: string;
}

export interface EpgSyncResult {
  channels_fetched: number;
  programs: number;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),

  listProviders: () => invoke<Provider[]>("list_providers"),
  addProvider: (input: AddProviderInput) => invoke<Provider>("add_provider", { input }),
  setProviderEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_provider_enabled", { id, enabled }),
  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),
  testConnection: (input: AddProviderInput) => invoke<TestResult>("test_connection", { input }),
  syncProvider: (id: number) => invoke<SyncResult>("sync_provider", { id }),

  curationSummary: (providerId: number) =>
    invoke<CountryGroup[]>("curation_summary", { providerId }),
  curationStats: (providerId: number) => invoke<CurationStats>("curation_stats", { providerId }),
  listCategories: (providerId: number) => invoke<Category[]>("list_categories", { providerId }),
  setCountryEnabled: (providerId: number, countryCode: string | null, enabled: boolean) =>
    invoke<number>("set_country_enabled", { providerId, countryCode, enabled }),
  setCategoryEnabled: (categoryId: number, enabled: boolean) =>
    invoke<void>("set_category_enabled", { categoryId, enabled }),
  setAllCategoriesEnabled: (providerId: number, enabled: boolean) =>
    invoke<number>("set_all_categories_enabled", { providerId, enabled }),

  listChannels: (opts: ChannelQueryOpts = {}) =>
    invoke<Channel[]>("list_channels", {
      providerId: opts.providerId ?? null,
      categoryId: opts.categoryId ?? null,
      search: opts.search ?? null,
      favoritesOnly: opts.favoritesOnly ?? false,
      limit: opts.limit ?? null,
    }),
  listActiveCategories: () => invoke<ActiveCategory[]>("list_active_categories"),
  setCategoriesEnabled: (categoryIds: number[], enabled: boolean) =>
    invoke<number>("set_categories_enabled", { categoryIds, enabled }),
  listRecent: (limit = 20) => invoke<Channel[]>("list_recent", { limit }),
  setFavorite: (providerId: number, streamId: number, favorite: boolean) =>
    invoke<void>("set_favorite", { providerId, streamId, favorite }),
  recordRecent: (providerId: number, streamId: number) =>
    invoke<void>("record_recent", { providerId, streamId }),
  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),

  syncEpg: (providerId: number) => invoke<EpgSyncResult>("sync_epg", { providerId }),
  getEpg: (providerId: number, from: number, to: number) =>
    invoke<EpgProgram[]>("get_epg", { providerId, from, to }),

  streamUrl: (providerId: number, streamId: number, container?: "m3u8" | "ts") =>
    invoke<string>("stream_url", { providerId, streamId, container: container ?? null }),
  launchExternal: (command: string, args: string[]) =>
    invoke<void>("launch_external", { command, args }),

  importProvidersFile: () => invoke<number>("import_providers_file"),
  exportProvidersFile: () => invoke<string>("export_providers_file"),
};
