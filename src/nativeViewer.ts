import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SourceConfig } from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const nativeViewerLabel = "cinefinder-viewer";
export const nativeViewerEvent = "cinefinder://viewer-state";

export interface ViewerBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface NativeViewerEvent {
  label: string;
  status: ViewerStatusText;
  url?: string | null;
  message?: string | null;
}

export type ViewerStatusText =
  | "Opening result page"
  | "Looking for player"
  | "Opening watch page"
  | "Ready"
  | "Could not auto-resolve, showing result page";

export interface NativeViewerOpenResult {
  finalUrl: string;
  resolved: boolean;
  status: ViewerStatusText;
}

export function isDesktopApp(): boolean {
  return typeof window !== "undefined" && typeof window.__TAURI_INTERNALS__ !== "undefined";
}

export async function openNativeViewer(
  url: string,
  bounds: ViewerBounds,
  source: SourceConfig,
  label = nativeViewerLabel
): Promise<NativeViewerOpenResult> {
  return invoke<NativeViewerOpenResult>("open_viewer_webview", {
    label,
    url,
    bounds,
    source
  });
}

export async function closeNativeViewer(label = nativeViewerLabel): Promise<void> {
  if (!isDesktopApp()) {
    return;
  }
  await invoke<void>("close_viewer_webview", { label }).catch(() => undefined);
}

export async function resizeNativeViewer(
  bounds: ViewerBounds,
  label = nativeViewerLabel
): Promise<void> {
  if (!isDesktopApp() || bounds.width < 8 || bounds.height < 8) {
    return;
  }
  await invoke<void>("resize_viewer_webview", { label, bounds }).catch(() => undefined);
}

export async function reloadNativeViewer(label = nativeViewerLabel): Promise<void> {
  await invoke<void>("reload_viewer_webview", { label });
}

export async function goBackNativeViewer(label = nativeViewerLabel): Promise<void> {
  await invoke<void>("go_back_viewer_webview", { label });
}

export async function goForwardNativeViewer(label = nativeViewerLabel): Promise<void> {
  await invoke<void>("go_forward_viewer_webview", { label });
}

export async function listenToNativeViewer(
  handler: (event: NativeViewerEvent) => void
): Promise<() => void> {
  return listen<NativeViewerEvent>(nativeViewerEvent, (event) => handler(event.payload));
}
