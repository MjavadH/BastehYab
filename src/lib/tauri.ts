import { invoke } from "@tauri-apps/api/core";

export interface AppHealth {
  status: "ok";
  appName: string;
}

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}
