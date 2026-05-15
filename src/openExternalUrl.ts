import { invoke } from "@tauri-apps/api/core";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export async function openExternalUrl(url: string): Promise<void> {
  if (!window.__TAURI_INTERNALS__) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }

  try {
    await invoke("open_external_url", { url });
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
