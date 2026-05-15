import { ArrowLeft, ExternalLink, Maximize2, MonitorPlay, RefreshCw, Star } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { isPlayableVideoUrl } from "../media";
import { openProviderWindow } from "../openProviderWindow";
import type { Favorite, SearchResult } from "../types";
import { cx } from "../utils";

interface ViewerViewProps {
  item: SearchResult | null;
  favorites: Favorite[];
  canGoBack: boolean;
  onBack: () => void;
  onOpenExternal: (result: SearchResult) => void;
  onToggleFavorite: (result: SearchResult) => void;
}

export function ViewerView({
  item,
  favorites,
  canGoBack,
  onBack,
  onOpenExternal,
  onToggleFavorite
}: ViewerViewProps) {
  const [frameLoaded, setFrameLoaded] = useState(false);
  const [frameKey, setFrameKey] = useState(0);
  const [launchState, setLaunchState] = useState<"idle" | "opening" | "opened" | "failed">("idle");
  const [launchError, setLaunchError] = useState<string | null>(null);

  useEffect(() => {
    setFrameLoaded(false);
    setFrameKey(0);
    setLaunchState("idle");
    setLaunchError(null);
  }, [item?.url]);

  if (!item) {
    return (
      <section className="view viewer-view">
        <header className="view-header">
          <div>
            <h1>Viewer</h1>
            <p>No result selected</p>
          </div>
        </header>
        <div className="empty-panel">
          <ExternalLink size={20} />
          <strong>Nothing open</strong>
          <span>Select a result from Search, Favorites, or History.</span>
        </div>
      </section>
    );
  }

  const isFavorite = favorites.some((favorite) => favorite.url === item.url);
  const playableUrl = item.playableUrl || (isPlayableVideoUrl(item.url) ? item.url : null);
  const shouldUseWebView = item.openMode === "webview" && !playableUrl;
  const isDesktopApp =
    typeof window !== "undefined" && typeof window.__TAURI_INTERNALS__ !== "undefined";
  const enterFullscreen = async () => {
    const shell = document.querySelector(".browser-shell");
    await shell?.requestFullscreen?.();
  };
  const launchDesktopWebView = useCallback(async () => {
    if (!item) {
      return;
    }
    setLaunchState("opening");
    setLaunchError(null);
    try {
      await openProviderWindow(item.url, item.title, { fallbackExternal: false });
      setLaunchState("opened");
    } catch (error) {
      setLaunchState("failed");
      setLaunchError(error instanceof Error ? error.message : String(error));
    }
  }, [item]);

  useEffect(() => {
    if (shouldUseWebView && isDesktopApp && launchState === "idle") {
      void launchDesktopWebView();
    }
  }, [isDesktopApp, launchDesktopWebView, launchState, shouldUseWebView]);

  return (
    <section className="view viewer-view">
      <header className="viewer-header">
        <div className="viewer-title-wrap">
          <button
            className="icon-button"
            type="button"
            onClick={onBack}
            disabled={!canGoBack}
            title="Back"
          >
            <ArrowLeft size={18} />
          </button>
          <div className="viewer-title">
            <span className="source-badge">{item.sourceName}</span>
            <h1>{item.title}</h1>
            <p>{item.url}</p>
          </div>
        </div>
        <div className="toolbar">
          {shouldUseWebView && (
            <button
              className="secondary-button"
              type="button"
              onClick={() => onOpenExternal(item)}
            >
              <ExternalLink size={17} />
              <span>Browser</span>
            </button>
          )}
          <button
            className={cx("icon-button", isFavorite && "is-on")}
            type="button"
            onClick={() => onToggleFavorite(item)}
            title={isFavorite ? "Remove favorite" : "Save favorite"}
          >
            <Star size={18} fill={isFavorite ? "currentColor" : "none"} />
          </button>
        </div>
      </header>

      {playableUrl ? (
        <div className="browser-shell">
          <video
            key={playableUrl}
            className="native-player"
            src={playableUrl}
            poster={item.posterUrl ?? undefined}
            controls
            playsInline
          />
        </div>
      ) : shouldUseWebView ? (
        <div className="browser-shell">
          <div className="webview-toolbar">
            <span>
              {isDesktopApp
                ? launchState === "opened"
                  ? "Opened in CineFinder WebView"
                  : launchState === "opening"
                    ? "Opening CineFinder WebView"
                    : launchState === "failed"
                      ? "Could not open app WebView"
                      : "Ready to open app WebView"
                : frameLoaded
                  ? "Loaded in browser preview"
                  : "Loading browser preview"}
            </span>
            <div className="toolbar">
              {!isDesktopApp && (
                <>
                  <button className="icon-button" type="button" onClick={() => void enterFullscreen()} title="Fullscreen">
                    <Maximize2 size={16} />
                  </button>
                  <button
                    className="icon-button"
                    type="button"
                    onClick={() => {
                      setFrameLoaded(false);
                      setFrameKey((current) => current + 1);
                    }}
                    title="Refresh"
                  >
                    <RefreshCw size={16} />
                  </button>
                </>
              )}
              {isDesktopApp && (
                <button className="secondary-button" type="button" onClick={() => void launchDesktopWebView()}>
                  <MonitorPlay size={17} />
                  <span>{launchState === "opened" ? "Reopen" : "Open app WebView"}</span>
                </button>
              )}
              <button className="secondary-button" type="button" onClick={() => onOpenExternal(item)}>
                <ExternalLink size={17} />
                <span>Browser</span>
              </button>
            </div>
          </div>
          {isDesktopApp ? (
            <div className="empty-panel provider-launch-panel">
              <MonitorPlay size={24} />
              <strong>
                {launchState === "opened"
                  ? "Provider page opened in a CineFinder WebView window"
                  : launchState === "opening"
                    ? "Opening provider page"
                    : launchState === "failed"
                      ? "Provider WebView did not open"
                      : "Open this provider in CineFinder"}
              </strong>
              <span>
                This uses a real Tauri WebView instead of an iframe, so sites that block embedding
                can still load inside the desktop app.
              </span>
              {launchError && <small>{launchError}</small>}
            </div>
          ) : (
            <iframe
              key={`${item.url}-${frameKey}`}
              src={item.url}
              title={item.title}
              allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
              allowFullScreen
              referrerPolicy="no-referrer"
              onLoad={() => setFrameLoaded(true)}
            />
          )}
        </div>
      ) : (
        <div className="empty-panel">
          <ExternalLink size={20} />
          <strong>No playable video URL</strong>
          <span>This item does not include a direct video file or stream.</span>
        </div>
      )}
    </section>
  );
}
