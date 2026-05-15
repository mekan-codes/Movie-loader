import { openExternalUrl } from "./openExternalUrl";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

interface OpenProviderWindowOptions {
  fallbackExternal?: boolean;
}

export async function openProviderWindow(
  url: string,
  title: string,
  options: OpenProviderWindowOptions = {}
): Promise<void> {
  const fallbackExternal = options.fallbackExternal ?? true;
  if (!window.__TAURI_INTERNALS__) {
    if (fallbackExternal) {
      await openExternalUrl(url);
      return;
    }
    throw new Error("Provider windows are only available in the desktop app.");
  }

  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const label = `provider_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
  const webview = new WebviewWindow(label, {
    url,
    title,
    width: 1240,
    height: 820,
    minWidth: 860,
    minHeight: 620,
    center: true,
    focus: true
  });

  try {
    await new Promise<void>((resolve, reject) => {
      void webview.once("tauri://created", () => resolve());
      void webview.once("tauri://error", (event) => reject(event.payload));
    });
  } catch (error) {
    if (fallbackExternal) {
      await openExternalUrl(url);
      return;
    }
    throw error;
  }
}
