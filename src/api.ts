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
  setCountryEnabled: (providerId: number, countryCode: string | null, enabled: boolean) =>
    invoke<number>("set_country_enabled", { providerId, countryCode, enabled }),
};
