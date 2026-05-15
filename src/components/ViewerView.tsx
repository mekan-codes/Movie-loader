import { ArrowLeft, ExternalLink, Maximize2, RefreshCw, Star } from "lucide-react";
import { useEffect, useState } from "react";
import { isPlayableVideoUrl } from "../media";
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

  useEffect(() => {
    setFrameLoaded(false);
    setFrameKey(0);
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
  const enterFullscreen = async () => {
    const shell = document.querySelector(".browser-shell");
    await shell?.requestFullscreen?.();
  };

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
            <span>{frameLoaded ? "Loaded in app" : "Loading page"}</span>
            <div className="toolbar">
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
              <button className="secondary-button" type="button" onClick={() => onOpenExternal(item)}>
                <ExternalLink size={17} />
                <span>Browser</span>
              </button>
            </div>
          </div>
          <iframe
            key={`${item.url}-${frameKey}`}
            src={item.url}
            title={item.title}
            allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
            allowFullScreen
            referrerPolicy="no-referrer"
            onLoad={() => setFrameLoaded(true)}
          />
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
