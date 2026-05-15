import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "./api/client";
import { DownloadsView } from "./components/DownloadsView";
import { FavoritesView, HistoryView } from "./components/LibraryViews";
import { SearchView } from "./components/SearchView";
import { SettingsView } from "./components/SettingsView";
import { Sidebar } from "./components/Sidebar";
import { SourcesView } from "./components/SourcesView";
import { ViewerView } from "./components/ViewerView";
import { openExternalUrl } from "./openExternalUrl";
import type {
  AppExport,
  AppSettings,
  Favorite,
  HistoryItem,
  SearchResult,
  SourceConfig,
  SourceSearchOutcome,
  ViewKey
} from "./types";
import {
  defaultSettings,
  resultToFavorite,
  resultToHistory
} from "./utils";

const recentKey = "cinefinder.recentSearches";
const settingsKey = "cinefinder.settings";
const selectedSourcesKey = "cinefinder.selectedSources";

export default function App() {
  const [activeView, setActiveView] = useState<ViewKey>("search");
  const [viewStack, setViewStack] = useState<ViewKey[]>([]);
  const [query, setQuery] = useState("");
  const [sources, setSources] = useState<SourceConfig[]>([]);
  const [favorites, setFavorites] = useState<Favorite[]>([]);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [outcomes, setOutcomes] = useState<SourceSearchOutcome[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const searchingRef = useRef(false);
  const [viewerItem, setViewerItem] = useState<SearchResult | null>(null);
  const [recentSearches, setRecentSearches] = useState<string[]>(() =>
    readLocal(recentKey, [])
  );
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>(() =>
    readLocal(selectedSourcesKey, [])
  );
  const [settings, setSettings] = useState<AppSettings>(() => ({
    ...defaultSettings,
    ...readLocal<Partial<AppSettings>>(settingsKey, {})
  }));
  const webOpenDelayMs = Math.max(0, settings.webOpenDelaySeconds || 0) * 1000;

  const enabledSources = useMemo(
    () => sources.filter((source) => source.enabled && !source.hidden),
    [sources]
  );

  const navigateTo = useCallback((view: ViewKey) => {
    setActiveView((current) => {
      if (current === view) {
        return current;
      }
      setViewStack((stack) => [...stack, current].slice(-30));
      window.history.pushState({ cinefinderView: view }, "", window.location.href);
      return view;
    });
  }, []);

  const goBack = useCallback(() => {
    setViewStack((stack) => {
      const previousView = stack.at(-1);
      if (!previousView) {
        setActiveView("search");
        window.history.replaceState({ cinefinderView: "search" }, "", window.location.href);
        return [];
      }
      setActiveView(previousView);
      window.history.replaceState({ cinefinderView: previousView }, "", window.location.href);
      return stack.slice(0, -1);
    });
  }, []);

  useEffect(() => {
    void refreshData();
  }, []);

  useEffect(() => {
    window.history.replaceState({ cinefinderView: activeView }, "", window.location.href);
    const handlePopState = (event: PopStateEvent) => {
      const nextView = isViewKey(event.state?.cinefinderView)
        ? event.state.cinefinderView
        : "search";
      setActiveView(nextView);
      setViewStack((stack) => stack.slice(0, -1));
    };
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.altKey && event.key === "ArrowLeft") {
        event.preventDefault();
        goBack();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [goBack]);

  useEffect(() => {
    localStorage.setItem(recentKey, JSON.stringify(recentSearches));
  }, [recentSearches]);

  useEffect(() => {
    localStorage.setItem(settingsKey, JSON.stringify(settings));
    const applyTheme = () => {
      const resolved =
        settings.theme === "system"
          ? window.matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light"
          : settings.theme;
      document.documentElement.dataset.theme = resolved;
    };
    applyTheme();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [settings]);

  useEffect(() => {
    if (settings.defaultSearchBehavior === "lastSelected") {
      localStorage.setItem(selectedSourcesKey, JSON.stringify(selectedSourceIds));
    }
  }, [selectedSourceIds, settings.defaultSearchBehavior]);

  const refreshData = async () => {
    const [nextSources, nextFavorites, nextHistory] = await Promise.all([
      api.listSources(),
      api.listFavorites(),
      api.listHistory()
    ]);
    setSources(nextSources);
    setFavorites(nextFavorites);
    setHistory(nextHistory);
  };

  const refreshFavoritesAndHistory = async () => {
    const [nextFavorites, nextHistory] = await Promise.all([
      api.listFavorites(),
      api.listHistory()
    ]);
    setFavorites(nextFavorites);
    setHistory(nextHistory);
  };

  const runSearch = async (term = query) => {
    if (searchingRef.current) {
      return;
    }
    const trimmed = term.trim();
    if (!trimmed) {
      return;
    }

    setQuery(trimmed);
    searchingRef.current = true;
    setIsSearching(true);
    const sourceIds = selectedSourceIds.length > 0 ? selectedSourceIds : null;
    const preflightSources = [
      ...enabledSources.map((source) => ({
        id: source.id,
        name: source.name,
        baseUrl: source.baseUrl
      }))
    ].filter((source) => !sourceIds?.length || sourceIds.includes(source.id));
    setOutcomes(
      preflightSources.map((source) => ({
        sourceId: source.id,
        sourceName: source.name,
        status: "loading",
        message: "Loading page",
        elapsedMs: 0,
        results: []
      }))
    );

    setRecentSearches((current) =>
      [trimmed, ...current.filter((item) => item.toLowerCase() !== trimmed.toLowerCase())].slice(
        0,
        8
      )
    );

    try {
      const results = await api.searchSources(trimmed, sourceIds);
      setOutcomes(results);
    } catch (error) {
      setOutcomes(
        preflightSources.map((source) => ({
          sourceId: source.id,
          sourceName: source.name,
          status: "error",
          message: error instanceof Error ? error.message : String(error),
          elapsedMs: 0,
          results: []
        }))
      );
    } finally {
      searchingRef.current = false;
      setIsSearching(false);
    }
  };

  const openResult = async (result: SearchResult) => {
    await copyResultQueryIfNeeded(result);
    if (settings.openBehavior === "external") {
      await openExternalUrl(result.url);
    } else {
      setViewerItem(result);
      navigateTo("viewer");
    }
    await api.recordHistory(resultToHistory(result));
    await refreshFavoritesAndHistory();
  };

  const retrySourceSearch = async (sourceId: string) => {
    const trimmed = query.trim();
    if (!trimmed || searchingRef.current) {
      return;
    }

    searchingRef.current = true;
    setIsSearching(true);
    setOutcomes((current) =>
      current.map((outcome) =>
        outcome.sourceId === sourceId
          ? {
              ...outcome,
              status: "loading",
              message: "Retrying source",
              elapsedMs: 0,
              results: []
            }
          : outcome
      )
    );

    try {
      const [result] = await api.searchSources(trimmed, [sourceId]);
      if (result) {
        setOutcomes((current) =>
          current.map((outcome) => (outcome.sourceId === sourceId ? result : outcome))
        );
      }
    } catch (error) {
      setOutcomes((current) =>
        current.map((outcome) =>
          outcome.sourceId === sourceId
            ? {
                ...outcome,
                status: "error",
                message: error instanceof Error ? error.message : String(error),
                elapsedMs: 0,
                results: []
              }
            : outcome
        )
      );
    } finally {
      searchingRef.current = false;
      setIsSearching(false);
    }
  };

  const openExternalResult = async (result: SearchResult) => {
    await copyResultQueryIfNeeded(result);
    await openExternalUrl(result.url);
  };

  const openPrimaryWebResults = async () => {
    const results = outcomes
      .flatMap((outcome) => outcome.results)
      .filter((result) => result.openMode === "webview")
      .filter((result) => result.rawData?.primary !== "false")
      .slice(0, 12);

    for (const [index, result] of results.entries()) {
      if (index > 0 && webOpenDelayMs > 0) {
        await wait(webOpenDelayMs);
      }
      await copyResultQueryIfNeeded(result);
      await openExternalUrl(result.url).catch(() => undefined);
    }
  };

  const toggleFavorite = async (result: SearchResult) => {
    const favorite = favorites.find((item) => item.url === result.url);
    if (favorite) {
      await api.removeFavorite(favorite.id);
    } else {
      await api.addFavorite(resultToFavorite(result));
    }
    await refreshFavoritesAndHistory();
  };

  const importSources = async (incoming: SourceConfig[]) => {
    for (const source of incoming) {
      await api.saveSource(source);
    }
    setSources(await api.listSources());
  };

  const exportData = (): AppExport => ({
    version: 1,
    exportedAt: new Date().toISOString(),
    sources,
    favorites,
    history,
    settings
  });

  const importData = async (data: AppExport) => {
    if (!data || data.version !== 1) {
      throw new Error("Unsupported CineFinder data export.");
    }
    await importSources(data.sources || []);
    for (const favorite of data.favorites || []) {
      await api.addFavorite(favorite);
    }
    for (const item of data.history || []) {
      await api.recordHistory(item);
    }
    setSettings({ ...defaultSettings, ...(data.settings || {}) });
    await refreshData();
  };

  const setSelectedSources = (ids: string[]) => {
    setSelectedSourceIds(ids);
    if (settings.defaultSearchBehavior === "lastSelected") {
      localStorage.setItem(selectedSourcesKey, JSON.stringify(ids));
    }
  };

  const clearCache = () => {
    localStorage.removeItem(recentKey);
    setRecentSearches([]);
  };

  return (
    <div className="app-shell">
      <Sidebar
        activeView={activeView}
        onViewChange={navigateTo}
        sourcesCount={sources.length}
        favoritesCount={favorites.length}
        historyCount={history.length}
      />

      <main className="main-shell">
        {activeView === "search" && (
          <SearchView
            query={query}
            onQueryChange={setQuery}
            onSearch={(term) => void runSearch(term)}
            isSearching={isSearching}
            sources={sources}
            builtInSources={[]}
            selectedSourceIds={selectedSourceIds}
            onSelectedSourceIdsChange={setSelectedSources}
            recentSearches={recentSearches}
            outcomes={outcomes}
            favorites={favorites}
            onOpenResult={(result) => void openResult(result)}
            onOpenExternalResult={(result) => void openExternalResult(result)}
            onOpenPrimaryWebResults={() => void openPrimaryWebResults()}
            onRetrySource={(sourceId) => void retrySourceSearch(sourceId)}
            showBulkOpen={settings.openBehavior === "external"}
            onToggleFavorite={(result) => void toggleFavorite(result)}
          />
        )}

        {activeView === "sources" && (
          <SourcesView
            sources={sources}
            onSave={async (source) => {
              await api.saveSource(source);
              setSources(await api.listSources());
            }}
            onDelete={async (sourceId) => {
              await api.deleteSource(sourceId);
              setSources(await api.listSources());
            }}
            onTest={api.testSource}
            onImport={importSources}
            onDuplicate={async (source) => {
              await api.duplicateSource(source);
              setSources(await api.listSources());
            }}
            onResetSource={async (sourceId) => {
              await api.resetDefaultSource(sourceId);
              setSources(await api.listSources());
            }}
            onRestoreDefaults={async () => {
              setSources(await api.restoreDefaultSources());
            }}
            onResetAllDefaults={async () => {
              setSources(await api.resetAllDefaultSources());
            }}
          />
        )}

        {activeView === "viewer" && (
          <ViewerView
            item={viewerItem}
            favorites={favorites}
            canGoBack={viewStack.length > 0}
            onBack={goBack}
            onOpenExternal={(result) => void openExternalResult(result)}
            onToggleFavorite={(result) => void toggleFavorite(result)}
          />
        )}

        {activeView === "favorites" && (
          <FavoritesView
            favorites={favorites}
            onOpen={(result) => void openResult(result)}
            onRemove={(favoriteId) => {
              void api.removeFavorite(favoriteId).then(refreshFavoritesAndHistory);
            }}
          />
        )}

        {activeView === "history" && (
          <HistoryView
            history={history}
            onOpen={(result) => void openResult(result)}
            onRemove={(historyId) => {
              void api.removeHistoryItem(historyId).then(refreshFavoritesAndHistory);
            }}
            onClear={() => {
              void api.clearHistory().then(refreshFavoritesAndHistory);
            }}
          />
        )}

        {activeView === "downloads" && <DownloadsView />}

        {activeView === "settings" && (
          <SettingsView
            settings={settings}
            onSettingsChange={setSettings}
            onClearCache={clearCache}
            onExportData={exportData}
            onImportData={importData}
          />
        )}
      </main>
    </div>
  );
}

async function copyResultQueryIfNeeded(result: SearchResult): Promise<void> {
  const value = result.rawData?.copyQuery;
  if (!value || !navigator.clipboard?.writeText) {
    return;
  }
  await navigator.clipboard.writeText(value).catch(() => undefined);
}

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function readLocal<T>(key: string, fallback: T): T {
  try {
    const value = localStorage.getItem(key);
    return value ? (JSON.parse(value) as T) : fallback;
  } catch {
    return fallback;
  }
}

function isViewKey(value: unknown): value is ViewKey {
  return (
    value === "search" ||
    value === "sources" ||
    value === "viewer" ||
    value === "favorites" ||
    value === "history" ||
    value === "downloads" ||
    value === "settings"
  );
}
