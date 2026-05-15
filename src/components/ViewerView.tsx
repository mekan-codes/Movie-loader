import {
  ArrowLeft,
  ArrowRight,
  ExternalLink,
  Maximize2,
  RefreshCw,
  Star
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { isPlayableVideoUrl } from "../media";
import {
  closeNativeViewer,
  goBackNativeViewer,
  goForwardNativeViewer,
  isDesktopApp,
  listenToNativeViewer,
  nativeViewerLabel,
  openNativeViewer,
  reloadNativeViewer,
  resizeNativeViewer,
  type ViewerBounds,
  type ViewerStatusText
} from "../nativeViewer";
import type { Favorite, SearchResult, SourceConfig } from "../types";
import { createId, cx, normalizeSource } from "../utils";

interface ViewerViewProps {
  item: SearchResult | null;
  source?: SourceConfig | null;
  favorites: Favorite[];
  canGoBack: boolean;
  onBack: () => void;
  onOpenExternal: (result: SearchResult) => void;
  onToggleFavorite: (result: SearchResult) => void;
}

export function ViewerView({
  item,
  source,
  favorites,
  canGoBack,
  onBack,
  onOpenExternal,
  onToggleFavorite
}: ViewerViewProps) {
  const [frameLoaded, setFrameLoaded] = useState(false);
  const [frameKey, setFrameKey] = useState(0);
  const [viewerStatus, setViewerStatus] = useState<ViewerStatusText>("Opening result page");
  const [currentUrl, setCurrentUrl] = useState(item?.url ?? "");
  const [nativeError, setNativeError] = useState<string | null>(null);
  const [nativeOpening, setNativeOpening] = useState(false);
  const hostRef = useRef<HTMLDivElement | null>(null);
  const lastBoundsRef = useRef<ViewerBounds | null>(null);

  const playableUrl = item?.playableUrl || (item && isPlayableVideoUrl(item.url) ? item.url : null);
  const shouldUseWebView = Boolean(item && item.openMode === "webview" && !playableUrl);
  const desktopApp = isDesktopApp();
  const viewerSource = useMemo(
    () => (item ? normalizeSource(source ?? fallbackSourceForItem(item)) : null),
    [item, source]
  );

  useEffect(() => {
    setFrameLoaded(false);
    setFrameKey(0);
    setCurrentUrl(item?.url ?? "");
    setViewerStatus("Opening result page");
    setNativeError(null);
    setNativeOpening(false);
  }, [item?.url]);

  useEffect(() => {
    if (!desktopApp) {
      return;
    }
    let mounted = true;
    let unlisten: (() => void) | null = null;
    void listenToNativeViewer((event) => {
      if (!mounted || event.label !== nativeViewerLabel) {
        return;
      }
      setViewerStatus(event.status);
      if (event.url) {
        setCurrentUrl(event.url);
      }
      if (event.message) {
        setNativeError(event.message);
      }
    }).then((nextUnlisten) => {
      unlisten = nextUnlisten;
      if (!mounted) {
        nextUnlisten();
      }
    });
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, [desktopApp]);

  const readHostBounds = useCallback((): ViewerBounds | null => {
    const host = hostRef.current;
    if (!host) {
      return null;
    }
    const rect = host.getBoundingClientRect();
    if (rect.width < 8 || rect.height < 8) {
      return null;
    }
    return {
      x: Math.round(rect.left),
      y: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height)
    };
  }, []);

  useEffect(() => {
    if (!desktopApp || !shouldUseWebView || !item || !viewerSource) {
      void closeNativeViewer();
      return;
    }

    let cancelled = false;
    let openTimer = 0;

    const openWhenMeasured = () => {
      const bounds = readHostBounds();
      if (!bounds) {
        openTimer = window.setTimeout(openWhenMeasured, 80);
        return;
      }
      lastBoundsRef.current = bounds;
      setViewerStatus("Opening result page");
      setNativeOpening(true);
      void openNativeViewer(item.url, bounds, viewerSource)
        .then((result) => {
          if (cancelled) {
            void closeNativeViewer();
            return;
          }
          setNativeOpening(false);
          setViewerStatus(result.status);
          setCurrentUrl(result.finalUrl);
        })
        .catch((error) => {
          if (!cancelled) {
            setNativeOpening(false);
            setViewerStatus("Could not auto-resolve, showing result page");
            setNativeError(error instanceof Error ? error.message : String(error));
          }
        });
    };

    openWhenMeasured();

    const updateNativeBounds = () => {
      const bounds = readHostBounds();
      if (!bounds) {
        return;
      }
      lastBoundsRef.current = bounds;
      void resizeNativeViewer(bounds);
    };
    const observer = new ResizeObserver(updateNativeBounds);
    if (hostRef.current) {
      observer.observe(hostRef.current);
    }
    window.addEventListener("resize", updateNativeBounds);

    return () => {
      cancelled = true;
      window.clearTimeout(openTimer);
      observer.disconnect();
      window.removeEventListener("resize", updateNativeBounds);
      void closeNativeViewer();
    };
  }, [desktopApp, item, readHostBounds, shouldUseWebView, viewerSource]);

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
  const externalTarget = { ...item, url: currentUrl || item.url };
  const visibleViewerStatus: ViewerStatusText =
    nativeOpening && viewerStatus === "Ready" && viewerSource?.autoResolveWatchPage !== false
      ? "Looking for player"
      : viewerStatus;

  const enterFullscreen = async () => {
    const shell = document.querySelector(".browser-shell");
    await shell?.requestFullscreen?.();
    const bounds = readHostBounds();
    if (bounds) {
      await resizeNativeViewer(bounds);
    }
  };

  const refreshViewer = () => {
    setViewerStatus("Opening result page");
    if (desktopApp && shouldUseWebView) {
      void reloadNativeViewer().catch((error) =>
        setNativeError(error instanceof Error ? error.message : String(error))
      );
      return;
    }
    setFrameLoaded(false);
    setFrameKey((current) => current + 1);
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
            <p>{currentUrl || item.url}</p>
          </div>
        </div>
        <div className="toolbar">
          {shouldUseWebView && (
            <button
              className="secondary-button"
              type="button"
              onClick={() => onOpenExternal(externalTarget)}
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
              {desktopApp ? visibleViewerStatus : frameLoaded ? "Ready" : "Opening result page"}
            </span>
            <div className="toolbar">
              {desktopApp && (
                <>
                  <button
                    className="icon-button"
                    type="button"
                    onClick={() => void goBackNativeViewer()}
                    title="Back in viewer"
                  >
                    <ArrowLeft size={16} />
                  </button>
                  <button
                    className="icon-button"
                    type="button"
                    onClick={() => void goForwardNativeViewer()}
                    title="Forward in viewer"
                  >
                    <ArrowRight size={16} />
                  </button>
                </>
              )}
              <button
                className="icon-button"
                type="button"
                onClick={() => void enterFullscreen()}
                title="Fullscreen"
              >
                <Maximize2 size={16} />
              </button>
              <button className="icon-button" type="button" onClick={refreshViewer} title="Refresh">
                <RefreshCw size={16} />
              </button>
              <button
                className="secondary-button"
                type="button"
                onClick={() => onOpenExternal(externalTarget)}
              >
                <ExternalLink size={17} />
                <span>Browser</span>
              </button>
            </div>
          </div>
          {desktopApp ? (
            <div className="native-webview-host" ref={hostRef}>
              {(nativeOpening || viewerStatus !== "Ready" || nativeError) && (
                <div className="viewer-loading-overlay">
                  <RefreshCw className="spin" size={18} />
                  <strong>{visibleViewerStatus}</strong>
                  {nativeError && <span>{nativeError}</span>}
                </div>
              )}
            </div>
          ) : (
            <iframe
              key={`${item.url}-${frameKey}`}
              src={item.url}
              title={item.title}
              allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
              allowFullScreen
              sandbox="allow-forms allow-modals allow-pointer-lock allow-presentation allow-same-origin allow-scripts"
              referrerPolicy="no-referrer"
              onLoad={() => {
                setFrameLoaded(true);
                setViewerStatus("Ready");
              }}
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

function fallbackSourceForItem(item: SearchResult): SourceConfig {
  const baseUrl = (() => {
    try {
      return new URL(item.url).origin;
    } catch {
      return item.url;
    }
  })();

  return {
    id: item.sourceId || createId("viewer-source"),
    name: item.sourceName || "Viewer source",
    enabled: true,
    isDefault: false,
    userModified: false,
    hidden: false,
    isDeleted: false,
    deletedAt: null,
    sourceKind: "web",
    sourceType: "directPage",
    sourceOpenBehavior: "webview",
    resultOpenBehavior: "result_page",
    ambiguousQueryBehavior: "show_choices",
    baseUrl,
    searchUrl: item.url,
    method: "GET",
    resultSelector: "",
    loadDelayMs: 1500,
    maxRetries: 2,
    requestTimeoutMs: 15000,
    waitForSelector: "",
    titleSelector: "",
    posterSelector: "",
    posterAttribute: "src",
    linkSelector: "",
    linkAttribute: "href",
    yearSelector: "",
    descriptionSelector: "",
    videoSelector: "",
    videoAttribute: "src",
    iframeSelector: "iframe",
    iframeAttribute: "src",
    subtitleSelector: "",
    subtitleAttribute: "src",
    subtitleLanguageAttribute: "srclang",
    audioLanguageSelector: "",
    downloadSelector: "",
    downloadAttribute: "href",
    watchButtonSelector: "",
    watchLinkTextPatterns: [
      "watch full movie",
      "watch online",
      "watch now",
      "play",
      "start watching",
      "смотреть",
      "смотреть онлайн"
    ],
    episodeSelector: "",
    seasonSelector: "",
    playerSelector: "video, iframe",
    autoResolveWatchPage: true,
    autoOpenFirstWatchLink: false,
    autoOpenBestMatch: true,
    autoOpenWatchButton: true,
    maxWatchResolveSteps: 2,
    maxResolveSteps: 2,
    resolveDelayMs: 1500,
    exactMatchThreshold: 85,
    requiresJavaScript: true,
    headers: {}
  };
}
