import { invoke } from "@tauri-apps/api/core";
import { distance as levenshteinDistance } from "fastest-levenshtein";
import { defaultSources, removedDefaultSourceIds } from "../data/samples";
import { isPlayableVideoUrl } from "../media";
import type {
  Favorite,
  HistoryItem,
  SelectorCandidate,
  SearchResult,
  SourceConfig,
  SourceSearchOutcome,
  SourceTestResult
} from "../types";
import { createId, normalizeSource, stableId } from "../utils";
import { extractYear, stripHtml } from "./builtinProviders";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const isTauri = () =>
  typeof window !== "undefined" && typeof window.__TAURI_INTERNALS__ !== "undefined";

const storageKeys = {
  sources: "cinefinder.sources",
  deletedDefaultSources: "cinefinder.deletedDefaultSources",
  favorites: "cinefinder.favorites",
  history: "cinefinder.history"
};

const commonResultSelectors = [
  "article",
  ".card",
  ".movie",
  ".movie-card",
  ".film",
  ".item",
  ".post",
  ".entry",
  ".video",
  ".poster",
  ".thumb",
  ".result",
  "a[href]"
];

const commonTitleSelectors = [
  "h1",
  "h2",
  "h3",
  "h4",
  ".title",
  ".name",
  ".movie-title",
  ".film-title",
  ".entry-title",
  "[title]",
  "img[alt]"
];

const commonLinkSelectors = [
  "a[href]",
  ".title a",
  ".movie-title a",
  ".poster a",
  ".thumb a",
  "article a",
  ".card a"
];

const commonPosterSelectors = ["img", ".poster img", ".thumb img", "picture img"];
const commonYearSelectors = [".year", ".date", ".release"];
const broadFranchiseQueries = new Set([
  "batman",
  "spider man",
  "spiderman",
  "superman",
  "x men",
  "xmen",
  "star wars",
  "star trek"
]);

export const api = {
  async listSources(): Promise<SourceConfig[]> {
    if (isTauri()) {
      return invoke<SourceConfig[]>("list_sources");
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    writeStorage(storageKeys.sources, sources);
    return sources;
  },

  async saveSource(source: SourceConfig): Promise<SourceConfig> {
    const normalized = normalizeSource({
      ...source,
      userModified: source.isDefault ? true : source.userModified
    });
    if (isTauri()) {
      return invoke<SourceConfig>("save_source", { source: normalized });
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    const now = new Date().toISOString();
    const saved = {
      ...normalized,
      createdAt: normalized.createdAt || now,
      updatedAt: now
    };
    writeStorage(
      storageKeys.sources,
      sources.some((item) => item.id === saved.id)
        ? sources.map((item) => (item.id === saved.id ? saved : item))
        : [...sources, saved]
    );
    return saved;
  },

  async deleteSource(sourceId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("delete_source", { sourceId });
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    const now = new Date().toISOString();
    writeStorage(
      storageKeys.sources,
      sources.map((source) =>
        source.id === sourceId
          ? {
              ...source,
              hidden: false,
              isDeleted: true,
              deletedAt: now,
              userModified: source.isDefault ? true : source.userModified
            }
          : source
      )
    );
  },

  async restoreSource(sourceId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("restore_source", { sourceId });
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    writeStorage(
      storageKeys.sources,
      sources.map((source) =>
        source.id === sourceId
          ? { ...source, hidden: false, isDeleted: false, deletedAt: null }
          : source
      )
    );
  },

  async permanentlyDeleteSource(sourceId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("permanently_delete_source", { sourceId });
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    const target = sources.find((source) => source.id === sourceId);
    if (target?.defaultSourceId) {
      addDeletedDefaultSourceId(target.defaultSourceId);
    }
    writeStorage(
      storageKeys.sources,
      sources.filter((source) => source.id !== sourceId)
    );
  },

  async emptySourceTrash(): Promise<void> {
    if (isTauri()) {
      return invoke<void>("empty_source_trash");
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    sources
      .filter((source) => source.isDeleted && source.defaultSourceId)
      .forEach((source) => addDeletedDefaultSourceId(source.defaultSourceId as string));
    writeStorage(
      storageKeys.sources,
      sources.filter((source) => !source.isDeleted)
    );
  },

  async duplicateSource(source: SourceConfig): Promise<SourceConfig> {
    const duplicate = normalizeSource({
      ...source,
      id: createId("source"),
      name: `${source.name} Copy`,
      isDefault: false,
      defaultSourceId: null,
      userModified: false,
      hidden: false,
      isDeleted: false,
      deletedAt: null,
      createdAt: null,
      updatedAt: null
    });
    return api.saveSource(duplicate);
  },

  async resetDefaultSource(sourceId: string): Promise<SourceConfig> {
    if (isTauri()) {
      return invoke<SourceConfig>("reset_default_source", { sourceId });
    }
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    const current = sources.find((source) => source.id === sourceId);
    const defaultSource = defaultSources.find(
      (source) => source.defaultSourceId === current?.defaultSourceId
    );
    if (!current || !defaultSource) {
      throw new Error("No built-in default config found for this source.");
    }
    const reset = normalizeSource({
      ...defaultSource,
      id: current.id,
      createdAt: current.createdAt,
      hidden: false,
      isDeleted: false,
      deletedAt: null,
      enabled: true,
      userModified: false,
      updatedAt: new Date().toISOString()
    });
    writeStorage(
      storageKeys.sources,
      sources.map((source) => (source.id === sourceId ? reset : source))
    );
    return reset;
  },

  async restoreDefaultSources(): Promise<SourceConfig[]> {
    if (isTauri()) {
      return invoke<SourceConfig[]>("restore_default_sources");
    }
    writeStorage(storageKeys.deletedDefaultSources, []);
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []), {
      restoreHidden: true
    });
    writeStorage(storageKeys.sources, sources);
    return sources;
  },

  async resetAllDefaultSources(): Promise<SourceConfig[]> {
    if (isTauri()) {
      return invoke<SourceConfig[]>("reset_all_default_sources");
    }
    writeStorage(storageKeys.deletedDefaultSources, []);
    const sources = migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
    const reset = sources.map((source) => {
      const defaultSource = defaultSources.find(
        (candidate) => candidate.defaultSourceId === source.defaultSourceId
      );
      return defaultSource
        ? normalizeSource({
            ...defaultSource,
            id: source.id,
            createdAt: source.createdAt,
            updatedAt: new Date().toISOString()
          })
        : source;
    });
    writeStorage(storageKeys.sources, reset);
    return reset;
  },

  async testSource(source: SourceConfig, query = "gravity falls"): Promise<SourceTestResult> {
    const normalized = normalizeSource(source);
    if (isTauri()) {
      return invoke<SourceTestResult>("test_source", {
        source: normalized,
        query
      });
    }
    if (normalized.sourceType === "webviewOnly" || normalized.sourceType === "directPage") {
      return testWebSource(normalized, query);
    }
    return testBrowserSource(normalized, query);
  },

  async searchSources(
    query: string,
    sourceIds: string[] | null
  ): Promise<SourceSearchOutcome[]> {
    return searchCustomSources(query, sourceIds);
  },

  async listFavorites(): Promise<Favorite[]> {
    if (isTauri()) {
      return invoke<Favorite[]>("list_favorites");
    }
    return readStorage<Favorite[]>(storageKeys.favorites, []);
  },

  async addFavorite(favorite: Favorite): Promise<Favorite> {
    if (isTauri()) {
      return invoke<Favorite>("add_favorite", { favorite });
    }
    const favorites = readStorage<Favorite[]>(storageKeys.favorites, []);
    const saved = { ...favorite, createdAt: favorite.createdAt || new Date().toISOString() };
    writeStorage(
      storageKeys.favorites,
      favorites.some((item) => item.url === saved.url)
        ? favorites.map((item) => (item.url === saved.url ? saved : item))
        : [saved, ...favorites]
    );
    return saved;
  },

  async removeFavorite(favoriteId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("remove_favorite", { favoriteId });
    }
    writeStorage(
      storageKeys.favorites,
      readStorage<Favorite[]>(storageKeys.favorites, []).filter((item) => item.id !== favoriteId)
    );
  },

  async listHistory(): Promise<HistoryItem[]> {
    if (isTauri()) {
      return invoke<HistoryItem[]>("list_history");
    }
    return readStorage<HistoryItem[]>(storageKeys.history, []);
  },

  async recordHistory(item: HistoryItem): Promise<HistoryItem> {
    if (isTauri()) {
      return invoke<HistoryItem>("record_history", { item });
    }
    const history = readStorage<HistoryItem[]>(storageKeys.history, []);
    const saved = { ...item, lastOpenedAt: new Date().toISOString() };
    writeStorage(
      storageKeys.history,
      [saved, ...history.filter((entry) => entry.url !== saved.url)].slice(0, 100)
    );
    return saved;
  },

  async removeHistoryItem(historyId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("remove_history_item", { historyId });
    }
    writeStorage(
      storageKeys.history,
      readStorage<HistoryItem[]>(storageKeys.history, []).filter((item) => item.id !== historyId)
    );
  },

  async clearHistory(): Promise<void> {
    if (isTauri()) {
      return invoke<void>("clear_history");
    }
    writeStorage(storageKeys.history, []);
  }
};

async function searchCustomSources(
  query: string,
  sourceIds: string[] | null
): Promise<SourceSearchOutcome[]> {
  const selectedSourceIds = sourceIds?.length ? sourceIds : null;

  const sources = (await listStoredSourcesForSearch())
    .map(normalizeSource)
    .filter((source) => source.enabled)
    .filter((source) => !source.hidden)
    .filter((source) => !source.isDeleted)
    .filter((source) => !selectedSourceIds?.length || selectedSourceIds.includes(source.id));

  return searchStoredSources(query, sources);
}

async function listStoredSourcesForSearch(): Promise<SourceConfig[]> {
  if (isTauri()) {
    return invoke<SourceConfig[]>("list_sources");
  }
  return migrateStoredSources(readStorage<SourceConfig[]>(storageKeys.sources, []));
}

async function searchStoredSources(
  query: string,
  sources: SourceConfig[]
): Promise<SourceSearchOutcome[]> {
  if (sources.length === 0) {
    return [];
  }

  if (isTauri()) {
    return invoke<SourceSearchOutcome[]>("search_sources", {
      query,
      sourceIds: sources.map((source) => source.id)
    });
  }

  const outcomes = await Promise.allSettled(
    sources.map((source) => searchBrowserSource(source, query))
  );

  return outcomes.map((outcome, index) => {
    if (outcome.status === "fulfilled") {
      return outcome.value;
    }
    const source = sources[index];
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: "error",
      message: outcome.reason instanceof Error ? outcome.reason.message : String(outcome.reason),
      elapsedMs: 0,
      results: []
    };
  });
}

function migrateStoredSources(
  storedSources: SourceConfig[],
  options: { restoreHidden?: boolean } = {}
): SourceConfig[] {
  const now = new Date().toISOString();
  const deletedDefaultIds = new Set(
    readStorage<string[]>(storageKeys.deletedDefaultSources, [])
  );
  const byDefaultId = new Map<string, SourceConfig>();
  const migrated = storedSources
    .map((source) => inferDefaultMetadata(normalizeSource(source)))
    .map((source) =>
      source.hidden && (source.isDefault || source.defaultSourceId)
        ? {
            ...source,
            hidden: false,
            isDeleted: true,
            deletedAt: source.deletedAt || now,
            userModified: true
          }
        : source
    )
    .filter((source) => !isWrongSource(source))
    .filter((source) => !isRemovedDefaultSource(source))
    .map((source) => {
      const defaultSource = defaultSources.find(
        (candidate) => candidate.defaultSourceId === source.defaultSourceId
      );
      if (!defaultSource) {
        return source;
      }
      if (source.userModified) {
        return options.restoreHidden && (source.hidden || source.isDeleted)
          ? { ...source, hidden: false, isDeleted: false, deletedAt: null, enabled: true }
          : source;
      }
      return normalizeSource({
        ...defaultSource,
        id: source.id,
        enabled: source.enabled,
        hidden: options.restoreHidden ? false : source.hidden,
        isDeleted: options.restoreHidden ? false : source.isDeleted,
        deletedAt: options.restoreHidden ? null : source.deletedAt,
        createdAt: source.createdAt,
        updatedAt: source.updatedAt || now
      });
    });

  migrated.forEach((source) => {
    if (source.defaultSourceId) {
      byDefaultId.set(source.defaultSourceId, source);
    }
  });

  for (const defaultSource of defaultSources) {
    if (
      !defaultSource.defaultSourceId ||
      byDefaultId.has(defaultSource.defaultSourceId) ||
      deletedDefaultIds.has(defaultSource.defaultSourceId)
    ) {
      continue;
    }
    migrated.push(
      normalizeSource({
        ...defaultSource,
        createdAt: now,
        updatedAt: now,
        hidden: false
      })
    );
  }

  return migrated.sort((left, right) => left.name.localeCompare(right.name));
}

function inferDefaultMetadata(source: SourceConfig): SourceConfig {
  const normalizedName = source.name.toLowerCase().trim();
  const match = defaultSources.find(
    (candidate) =>
      candidate.defaultSourceId === source.defaultSourceId ||
      candidate.name.toLowerCase() === normalizedName
  );
  if (!match) {
    return source;
  }
  return {
    ...source,
    defaultSourceId: source.defaultSourceId || match.defaultSourceId,
    isDefault: true
  };
}

function isWrongSource(source: SourceConfig): boolean {
  const normalizedName = source.name.toLowerCase();
  const normalizedBaseUrl = source.baseUrl.toLowerCase();
  return (
    /prada/i.test(source.name) ||
    /prada/i.test(source.baseUrl) ||
    (normalizedName.includes("example movie source") && normalizedBaseUrl.includes("example.com"))
  );
}

function isRemovedDefaultSource(source: SourceConfig): boolean {
  const normalizedName = source.name.toLowerCase();
  return (
    Boolean(source.isDefault && source.defaultSourceId && removedDefaultSourceIds.has(source.defaultSourceId)) ||
    (Boolean(source.isDefault) &&
      ["plex", "tubi", "pluto", "filmzie", "xumo", "filmrise", "arte"].some((name) =>
        normalizedName.includes(name)
      ))
  );
}

function testWebSource(source: SourceConfig, sampleQuery = "gravity falls"): SourceTestResult {
  const started = performance.now();
  try {
    new URL(source.baseUrl);
    const finalSearchUrl = buildSourceSearchUrl(source, sampleQuery);
    return {
      ok: true,
      message: "Web source is ready. Searches open in the in-app viewer.",
      resultCount: 1,
      elapsedMs: Math.round(performance.now() - started),
      finalSearchUrl,
      rawStatus: "loaded",
      selectorMatchCount: 1,
      previewResults: [{ title: `Provider card: ${source.name}`, url: finalSearchUrl }],
      fallbackUsed: true,
      querySpecificity: null,
      ambiguous: false,
      detectedSelectors: [],
      bestMatch: null,
      finalOpenUrl: finalSearchUrl
    };
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? error.message : String(error),
      resultCount: 0,
      elapsedMs: Math.round(performance.now() - started),
      finalSearchUrl: null,
      rawStatus: "failed",
      selectorMatchCount: 0,
      previewResults: [],
      fallbackUsed: false,
      querySpecificity: null,
      ambiguous: false,
      detectedSelectors: [],
      bestMatch: null,
      finalOpenUrl: null
    };
  }
}

function providerFallbackResult(
  source: SourceConfig,
  query: string,
  url: string,
  description = "Could not parse exact result. Open source search page."
): SearchResult {
  return {
    id: stableId(`${source.id}:${url}`, "result"),
    sourceId: source.id,
    sourceName: source.name,
    title: `Search "${query}" on ${source.name}`,
    url,
    openMode: "webview",
    playableUrl: null,
    posterUrl: null,
    year: null,
    description,
    confidence: 0,
    rawData: {
      provider: "fallback-provider",
      resultKind: "provider",
      primary: "true",
      resolution: "fallback"
    }
  };
}

function directPageResult(source: SourceConfig, query: string, url: string): SearchResult {
  const playableUrl = isPlayableVideoUrl(url) ? url : null;
  return {
    id: stableId(`${source.id}:${url}`, "result"),
    sourceId: source.id,
    sourceName: source.name,
    title: source.name || query,
    url,
    openMode: playableUrl ? "native" : "webview",
    playableUrl,
    posterUrl: null,
    year: null,
    description: "Configured direct page. Opens inside CineFinder.",
    confidence: 100,
    rawData: {
      provider: "direct-page",
      resultKind: "parsed",
      primary: "true",
      resolution: "exact",
      confidenceReason: "Configured direct page."
    }
  };
}

function isSelectorMissingError(error: unknown): boolean {
  return error instanceof Error && error.message === "No results or selector not found.";
}

async function testBrowserSource(
  source: SourceConfig,
  sampleQuery = "gravity falls"
): Promise<SourceTestResult> {
  const started = performance.now();
  const finalSearchUrl = buildSourceSearchUrl(source, sampleQuery);
  try {
    const html = await fetchBrowserSearchHtml(source, finalSearchUrl);
    const document = new DOMParser().parseFromString(html, "text/html");
    const detectedSelectors = detectSelectorCandidates(document);
    const parsedResults = parseBrowserResults(source, sampleQuery, finalSearchUrl, html);
    const decision = decideResolution(source, sampleQuery, parsedResults);
    const bestMatch = parsedResults[0] ?? null;
    const selectorMatchCount =
      detectedSelectors.find((candidate) => candidate.selectorType === "result")?.matchCount ??
      parsedResults.length;
    const previewResults = parsedResults
      .slice(0, 5)
      .map((result) => ({ title: result.title, url: result.url, year: result.year }));

    return {
      ok: true,
      message: bestMatch
        ? `${decision.specific ? "Specific" : "Ambiguous"} query. ${decision.reason}`
        : "Source loaded, but no matching result cards were parsed. Searches will use the fallback card.",
      resultCount: previewResults.length,
      elapsedMs: Math.round(performance.now() - started),
      finalSearchUrl,
      rawStatus: "loaded",
      selectorMatchCount,
      previewResults: previewResults.map((result) => ({
        ...result,
        score: parsedResults.find((candidate) => candidate.url === result.url)?.confidence,
        confidenceReason:
          parsedResults.find((candidate) => candidate.url === result.url)?.rawData?.confidenceReason ??
          null
      })),
      fallbackUsed: previewResults.length === 0,
      querySpecificity: decision.specificityReason,
      ambiguous: decision.ambiguous,
      detectedSelectors,
      bestMatch: bestMatch
        ? {
            title: bestMatch.title,
            url: bestMatch.url,
            year: bestMatch.year,
            score: bestMatch.confidence,
            confidenceReason: bestMatch.rawData?.confidenceReason
          }
        : null,
      finalOpenUrl: decision.selected?.url ?? finalSearchUrl
    };
  } catch (error) {
    return {
      ok: false,
      message: browserSourceErrorMessage(error),
      resultCount: 0,
      elapsedMs: Math.round(performance.now() - started),
      finalSearchUrl,
      rawStatus: error instanceof DOMException && error.name === "AbortError" ? "timed out" : "failed",
      selectorMatchCount: 0,
      previewResults: [],
      fallbackUsed: false,
      querySpecificity: null,
      ambiguous: false,
      detectedSelectors: [],
      bestMatch: null,
      finalOpenUrl: finalSearchUrl
    };
  }
}

function buildSourceSearchUrl(source: SourceConfig, query: string): string {
  const template = source.searchUrl.trim() || source.baseUrl.trim();
  if (!template) {
    throw new Error("Search URL is required.");
  }

  const encodedQuery = encodeURIComponent(query);
  const slugQuery = encodeURIComponent(query.trim().replace(/\s+/g, "-"));
  if (source.sourceType === "directPage") {
    return new URL(template, source.baseUrl).toString();
  }
  const rawUrl =
    template.includes("{query}") || template.includes("{slug}")
      ? template.replaceAll("{query}", encodedQuery).replaceAll("{slug}", slugQuery)
      : appendQueryParam(template, source.baseUrl, encodedQuery);
  return new URL(rawUrl, source.baseUrl).toString();
}

function appendQueryParam(value: string, baseUrl: string, encodedQuery: string): string {
  const url = new URL(value, baseUrl);
  url.searchParams.set("q", encodedQuery);
  return url.toString().replace(/q=([^&]*)/, () => `q=${encodedQuery}`);
}

async function searchBrowserSource(
  source: SourceConfig,
  query: string
): Promise<SourceSearchOutcome> {
  const started = performance.now();
  const searchUrl = buildSourceSearchUrl(source, query);

  if (source.sourceType === "webviewOnly") {
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: "ready",
      message: "WebView-only source. Open source search page.",
      elapsedMs: Math.round(performance.now() - started),
      results: [providerFallbackResult(source, query, searchUrl)]
    };
  }

  if (source.resultOpenBehavior === "search_page") {
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: "ready",
      message: "Configured to open the source search page.",
      elapsedMs: Math.round(performance.now() - started),
      results: [providerFallbackResult(source, query, searchUrl)]
    };
  }

  if (source.sourceType === "directPage") {
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: "found",
      message: "Direct page source. Opening configured page.",
      elapsedMs: Math.round(performance.now() - started),
      results: [directPageResult(source, query, searchUrl)]
    };
  }

  if (source.method.toUpperCase() !== "GET") {
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: "unsupported",
      message: "Only GET sources are supported in v1.",
      elapsedMs: 0,
      results: []
    };
  }

  try {
    const html = await fetchBrowserSearchHtml(source, searchUrl);
    const candidates = parseBrowserResults(source, query, searchUrl, html);
    const decision = decideResolution(source, query, candidates);
    if (decision.fallbackToSearch || decision.results.length === 0) {
      return {
        sourceId: source.id,
        sourceName: source.name,
        status: "ready",
        message: decision.reason,
        elapsedMs: Math.round(performance.now() - started),
        results: [providerFallbackResult(source, query, searchUrl, decision.reason)]
      };
    }
    const results = await resolveBrowserResultTargets(source, searchUrl, decision.results);
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: results.length > 0 ? "found" : "ready",
      message:
        results.length > 0
          ? decision.reason
          : "No parsed results. Open source search page.",
      elapsedMs: Math.round(performance.now() - started),
      results: results.length > 0 ? results : [providerFallbackResult(source, query, searchUrl)]
    };
  } catch (error) {
    const timedOut = error instanceof DOMException && error.name === "AbortError";
    if (isSelectorMissingError(error)) {
      return {
        sourceId: source.id,
        sourceName: source.name,
        status: "ready",
        message: "No parsed results. Open source search page.",
        elapsedMs: Math.round(performance.now() - started),
        results: [providerFallbackResult(source, query, searchUrl)]
      };
    }
    return {
      sourceId: source.id,
      sourceName: source.name,
      status: timedOut ? "timed_out" : "error",
      message: timedOut
        ? "Timed out after 15 seconds."
        : browserSourceErrorMessage(error),
      elapsedMs: Math.round(performance.now() - started),
      results: timedOut ? [] : [providerFallbackResult(source, query, searchUrl)]
    };
  }
}

async function fetchBrowserSearchHtml(source: SourceConfig, searchUrl: string): Promise<string> {
  const maxRetries = source.maxRetries ?? 2;
  const timeoutMs = source.requestTimeoutMs ?? 15000;
  const loadDelayMs = source.loadDelayMs ?? 1500;
  const waitSelector = source.waitForSelector || source.resultSelector;
  let lastError: unknown = null;

  for (let attempt = 0; attempt <= maxRetries; attempt += 1) {
    try {
      const controller = new AbortController();
      const timeout = window.setTimeout(() => controller.abort(), timeoutMs);
      let response: Response;
      try {
        response = await fetch(searchUrl, {
          headers: source.headers,
          signal: controller.signal
        });
      } finally {
        window.clearTimeout(timeout);
      }

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const html = await response.text();
      if (loadDelayMs > 0) {
        await wait(loadDelayMs);
      }

      if (waitSelector && selectorExists(html, waitSelector)) {
        return html;
      }
      if (!waitSelector) {
        return html;
      }

      lastError = new Error("No results or selector not found.");
    } catch (error) {
      lastError = error;
    }

    if (attempt < maxRetries) {
      await wait(Math.min(1000 + attempt * 500, 2500));
    }
  }

  throw lastError instanceof Error ? lastError : new Error(String(lastError));
}

function parseBrowserResults(
  source: SourceConfig,
  query: string,
  searchUrl: string,
  html: string
): SearchResult[] {
  const document = new DOMParser().parseFromString(html, "text/html");
  const resultSelectors = source.resultSelector.trim()
    ? [source.resultSelector.trim()]
    : commonResultSelectors;
  let bestResults: SearchResult[] = [];
  let bestScore = 0;

  for (const resultSelector of resultSelectors) {
    const results = parseBrowserResultsWithSelector(source, query, searchUrl, document, resultSelector);
    const strongestMatch = Math.max(0, ...results.map((result) => result.confidence));
    const selectorScore = strongestMatch + Math.min(results.length, 12);
    if (selectorScore > bestScore) {
      bestScore = selectorScore;
      bestResults = results;
    }
  }

  return bestResults.filter((result) => result.confidence >= 12).slice(0, 24);
}

function decideResolution(
  source: SourceConfig,
  query: string,
  candidates: SearchResult[]
): {
  specific: boolean;
  ambiguous: boolean;
  fallbackToSearch: boolean;
  selected: SearchResult | null;
  results: SearchResult[];
  reason: string;
  specificityReason: string;
} {
  const ranked = candidates
    .map((candidate) => {
      const scored = scoreResultCandidate(query, candidate.title, candidate.year);
      return {
        ...candidate,
        confidence: scored.score,
        rawData: {
          ...candidate.rawData,
          confidenceReason: scored.reason
        }
      };
    })
    .filter((candidate) => candidate.confidence >= 35)
    .sort((left, right) => right.confidence - left.confidence);

  const best = ranked[0] ?? null;
  const specificity = isSpecificQuery(query);
  if (!best) {
    return {
      specific: specificity.specific,
      ambiguous: false,
      fallbackToSearch: true,
      selected: null,
      results: [],
      reason: "Could not parse exact result. Open source search page.",
      specificityReason: specificity.reason
    };
  }

  const second = ranked[1] ?? null;
  const closeSecond = Boolean(second && best.confidence - second.confidence <= 8 && second.confidence >= 70);
  const threshold = source.exactMatchThreshold ?? 85;
  const yearInfo = extractQueryYear(query);
  const yearMismatch = Boolean(
    yearInfo && best.year && normalizeYear(best.year) && normalizeYear(best.year) !== yearInfo
  );
  const ambiguous =
    !specificity.specific ||
    closeSecond ||
    best.confidence < threshold ||
    yearMismatch ||
    source.autoOpenBestMatch === false;

  if (ambiguous) {
    if (source.ambiguousQueryBehavior === "open_search_page") {
      return {
        specific: specificity.specific,
        ambiguous: true,
        fallbackToSearch: true,
        selected: null,
        results: [],
        reason: yearMismatch
          ? "Best result year does not match query year. Open source search page."
          : "Multiple possible matches. Open source search page.",
        specificityReason: specificity.reason
      };
    }
    const choices = ranked.slice(0, 5).map((result) => ({
      ...result,
      rawData: {
        ...result.rawData,
        resolution: "ambiguous",
        querySpecificity: specificity.reason
      }
    }));
    return {
      specific: specificity.specific,
      ambiguous: true,
      fallbackToSearch: false,
      selected: best,
      results: choices,
      reason: yearMismatch
        ? "Multiple possible matches; best year does not match the query."
        : `Multiple possible matches. ${specificity.reason}`,
      specificityReason: specificity.reason
    };
  }

  return {
    specific: true,
    ambiguous: false,
    fallbackToSearch: false,
    selected: best,
    results: [
      {
        ...best,
        rawData: {
          ...best.rawData,
          resolution: "exact",
          querySpecificity: specificity.reason
        }
      }
    ],
    reason: `Movie page found. ${best.rawData?.confidenceReason || "Strong title match."}`,
    specificityReason: specificity.reason
  };
}

function isSpecificQuery(query: string): { specific: boolean; reason: string } {
  const normalized = normalizeComparableTitle(query);
  if (extractQueryYear(query)) {
    return { specific: true, reason: "Query includes a year." };
  }
  if (/\b(s\d{1,2}|season\s+\d{1,2}|episode\s+\d{1,3}|ep\s*\d{1,3})\b/i.test(query)) {
    return { specific: true, reason: "Query includes season or episode detail." };
  }
  if (broadFranchiseQueries.has(normalized)) {
    return { specific: false, reason: "Broad franchise query." };
  }
  if (normalized.split(" ").length >= 2) {
    return { specific: true, reason: "Query is a specific title phrase." };
  }
  return { specific: false, reason: "Short broad query." };
}

function scoreResultCandidate(
  query: string,
  title: string,
  candidateYear?: string | null
): { score: number; reason: string } {
  const queryYear = extractQueryYear(query);
  const resultYear = normalizeYear(candidateYear) || extractQueryYear(title);
  const queryTitle = normalizeComparableTitle(query.replace(/\b(19|20)\d{2}\b/g, " "));
  const resultTitle = normalizeComparableTitle(title.replace(/\b(19|20)\d{2}\b/g, " "));
  if (!queryTitle || !resultTitle) {
    return { score: 0, reason: "Missing title text." };
  }

  let score = 0;
  let reason = "Low title similarity.";
  if (queryTitle === resultTitle && queryYear && resultYear === queryYear) {
    score = 100;
    reason = "Exact title and year match.";
  } else if (queryTitle === resultTitle) {
    score = queryYear ? 82 : 90;
    reason = queryYear ? "Exact title, but year is missing." : "Exact title match.";
  } else if (resultTitle.includes(queryTitle)) {
    score = queryYear && resultYear === queryYear ? 85 : 80;
    reason = queryYear && resultYear === queryYear
      ? "Title contains query and year matches."
      : "Title contains query.";
  } else if (queryTitle.includes(resultTitle) && resultTitle.length >= 4) {
    score = 68;
    reason = "Query contains candidate title.";
  } else {
    const overlap = wordOverlapScore(queryTitle, resultTitle);
    const fuzzy = fuzzyTitleScore(queryTitle, resultTitle);
    score = Math.max(overlap, fuzzy);
    reason =
      fuzzy >= 78 && fuzzy >= overlap
        ? "High fuzzy title similarity."
        : overlap >= 70
          ? "High word overlap."
          : "Partial title overlap.";
  }

  if (queryYear && resultYear === queryYear) {
    score = Math.min(100, score + 8);
    reason = `${reason} Year matches.`;
  } else if (queryYear && resultYear && resultYear !== queryYear) {
    score = Math.min(score - 35, 60);
    reason = `${reason} Year differs from query.`;
  }

  return { score: Math.max(0, Math.round(score)), reason };
}

function fuzzyTitleScore(queryTitle: string, resultTitle: string): number {
  const maxLength = Math.max(queryTitle.length, resultTitle.length);
  if (maxLength === 0) {
    return 0;
  }
  const normalized = 1 - levenshteinDistance(queryTitle, resultTitle) / maxLength;
  const score = Math.round(normalized * 100);
  return score >= 55 ? score : Math.round(score * 0.65);
}

function wordOverlapScore(queryTitle: string, resultTitle: string): number {
  const queryTokens = tokenSet(queryTitle);
  const resultTokens = tokenSet(resultTitle);
  if (queryTokens.size === 0 || resultTokens.size === 0) {
    return 0;
  }
  let overlap = 0;
  queryTokens.forEach((token) => {
    if (resultTokens.has(token)) {
      overlap += 1;
    }
  });
  const queryCoverage = overlap / queryTokens.size;
  const resultCoverage = overlap / resultTokens.size;
  return Math.round(Math.max(queryCoverage * 82, resultCoverage * 68));
}

function tokenSet(value: string): Set<string> {
  return new Set(value.split(" ").filter((token) => token.length > 1));
}

function normalizeComparableTitle(value: string): string {
  return value
    .toLowerCase()
    .replace(/\b(i)\b/g, "1")
    .replace(/\b(ii)\b/g, "2")
    .replace(/\b(iii)\b/g, "3")
    .replace(/\b(iv)\b/g, "4")
    .replace(/\b(v)\b/g, "5")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .replace(/\b(full|movie|watch|online|hd|free|season|episode|ep|show|film|смотреть|онлайн)\b/gu, " ")
    .replace(/\b(смотреть)\b/gu, " ")
    .replace(/\b(s\d{1,2}|e\d{1,3}|ep\d{1,3})\b/gu, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function extractQueryYear(value: string): string | null {
  return normalizeYear(extractYear(value));
}

function normalizeYear(value?: string | null): string | null {
  const match = value?.match(/\b(19|20)\d{2}\b/);
  return match?.[0] ?? null;
}

function parseBrowserResultsWithSelector(
  source: SourceConfig,
  query: string,
  searchUrl: string,
  document: Document,
  resultSelector: string
): SearchResult[] {
  let cards: Element[];
  try {
    cards = Array.from(document.querySelectorAll(resultSelector)).slice(0, 100);
  } catch {
    return [];
  }
  const seen = new Set<string>();

  return cards
    .flatMap<SearchResult>((card, index) => {
      const linkElement = firstMatchingElement(
        card,
        source.linkSelector ? [source.linkSelector] : commonLinkSelectors
      ) ?? (card.matches("a[href]") ? card : null);
      const rawLink = readAttribute(linkElement, source.linkAttribute || "href");
      const url = absoluteUrl(source.baseUrl, searchUrl, rawLink);
      if (!url) {
        return [];
      }

      const title = inferBrowserTitle(card, source, linkElement) || url;
      const year =
        readText(source.yearSelector ? card.querySelector(source.yearSelector) : null) ||
        readText(firstMatchingElement(card, commonYearSelectors)) ||
        extractYear(`${title} ${readText(card) || ""}`);
      const description = readText(
        source.descriptionSelector ? card.querySelector(source.descriptionSelector) : null
      );
      const posterElement = firstMatchingElement(
        card,
        source.posterSelector ? [source.posterSelector] : commonPosterSelectors
      );
      const posterUrl = absoluteUrl(
        source.baseUrl,
        searchUrl,
        readAttribute(posterElement, source.posterAttribute || "src")
      );
      const videoElement = source.videoSelector
        ? card.querySelector(source.videoSelector)
        : null;
      const playableUrl =
        absoluteUrl(
          source.baseUrl,
          searchUrl,
          readAttribute(videoElement, source.videoAttribute || "src")
        ) ?? (isPlayableVideoUrl(url) ? url : null);

      const key = `${source.id}:${title.toLowerCase()}:${year || ""}`;
      if (seen.has(key)) {
        return [];
      }
      seen.add(key);
      const scored = scoreResultCandidate(query, title, year);
      const confidence = scored.score;

      return [
        {
          id: `${source.id}-${index}`,
          sourceId: source.id,
          sourceName: source.name,
          title,
          url,
          openMode: playableUrl ? "native" : "webview",
          playableUrl,
          posterUrl,
          year,
          description,
          confidence,
          rawData: {
            provider: "custom-browser",
            rank: String(index + 1),
            resultSelector,
            confidenceReason: scored.reason,
            querySpecificity: isSpecificQuery(query).reason
          }
        }
      ];
    })
    .sort((left, right) => right.confidence - left.confidence);
}

async function resolveBrowserResultTargets(
  source: SourceConfig,
  searchUrl: string,
  candidates: SearchResult[]
): Promise<SearchResult[]> {
  const limitedCandidates = candidates.slice(0, 30);
  const resolved = await Promise.all(
    limitedCandidates.map(async (result) => {
      if (result.playableUrl) {
        return result;
      }
      const watchUrl = await resolveBrowserWatchUrl(source, result.url);
      const resultPage = watchUrl
        ? {
            ...result,
            url: watchUrl,
            rawData: {
              ...result.rawData,
              detailPageUrl: result.url,
              openedVia: "watchButtonSelector"
            }
          }
        : result;
      if (!source.videoSelector) {
        return { ...resultPage, openMode: "webview" as const };
      }

      try {
        const controller = new AbortController();
        const timeout = window.setTimeout(() => controller.abort(), 15000);
        const response = await fetch(resultPage.url, {
          headers: source.headers,
          signal: controller.signal
        });
        window.clearTimeout(timeout);
        if (!response.ok) {
          return { ...resultPage, openMode: "webview" as const };
        }

        const html = await response.text();
        const document = new DOMParser().parseFromString(html, "text/html");
        const videoElement = document.querySelector(source.videoSelector);
        const playableUrl = absoluteUrl(
          source.baseUrl,
          resultPage.url || searchUrl,
          readAttribute(videoElement, source.videoAttribute || "src")
        );
        return playableUrl
          ? { ...resultPage, openMode: "native" as const, playableUrl }
          : { ...resultPage, openMode: "webview" as const };
      } catch {
        return { ...resultPage, openMode: "webview" as const };
      }
    })
  );

  return resolved.filter((result): result is SearchResult => Boolean(result));
}

function firstMatchingElement(parent: Element, selectors: string[]): Element | null {
  for (const selector of selectors) {
    try {
      if (parent.matches(selector)) {
        return parent;
      }
      const element = parent.querySelector(selector);
      if (element) {
        return element;
      }
    } catch {
      // Ignore selector candidates that do not parse in the current browser.
    }
  }
  return null;
}

function inferBrowserTitle(
  card: Element,
  source: SourceConfig,
  linkElement: Element | null
): string | null {
  const configuredTitle = source.titleSelector
    ? titleFromElement(firstMatchingElement(card, [source.titleSelector]))
    : null;
  if (configuredTitle) {
    return configuredTitle;
  }

  for (const selector of commonTitleSelectors) {
    const title = titleFromElement(firstMatchingElement(card, [selector]));
    if (title) {
      return title;
    }
  }

  return (
    titleFromElement(linkElement) ||
    titleFromElement(firstMatchingElement(card, ["img"])) ||
    titleFromElement(card)
  );
}

function titleFromElement(element: Element | null): string | null {
  if (!element) {
    return null;
  }
  return (
    readAttribute(element, "title") ||
    readAttribute(element, "aria-label") ||
    readAttribute(element, "alt") ||
    readAttribute(element.querySelector("img"), "alt") ||
    readText(element)
  );
}

async function resolveBrowserWatchUrl(
  source: SourceConfig,
  resultUrl: string
): Promise<string | null> {
  const canAutoResolve =
    source.autoResolveWatchPage !== false &&
    (Boolean(source.watchButtonSelector) ||
      source.autoOpenWatchButton ||
      source.autoOpenFirstWatchLink);
  if (!canAutoResolve) {
    return null;
  }

  let currentUrl = resultUrl;
  const maxSteps = Math.max(
    0,
    Math.min(source.maxResolveSteps ?? source.maxWatchResolveSteps ?? 2, 5)
  );
  try {
    for (let step = 0; step < maxSteps; step += 1) {
      const controller = new AbortController();
      const timeout = window.setTimeout(() => controller.abort(), source.requestTimeoutMs ?? 15000);
      const response = await fetch(currentUrl, {
        headers: source.headers,
        signal: controller.signal
      });
      window.clearTimeout(timeout);
      if (!response.ok) {
        return currentUrl === resultUrl ? null : currentUrl;
      }
      const html = await response.text();
      const document = new DOMParser().parseFromString(html, "text/html");
      if (source.playerSelector && document.querySelector(source.playerSelector)) {
        return currentUrl === resultUrl ? null : currentUrl;
      }

      const nextUrl = findWatchLinkUrl(source, document, currentUrl);
      if (!nextUrl || nextUrl === currentUrl) {
        return currentUrl === resultUrl ? null : currentUrl;
      }
      currentUrl = nextUrl;
    }
    return currentUrl === resultUrl ? null : currentUrl;
  } catch {
    return null;
  }
}

function findWatchLinkUrl(
  source: SourceConfig,
  document: Document,
  pageUrl: string
): string | null {
  if (source.watchButtonSelector) {
    const element = document.querySelector(source.watchButtonSelector);
    const url = elementUrl(source, element, pageUrl);
    if (url) {
      return url;
    }
  }

  if (!source.autoOpenWatchButton && !source.autoOpenFirstWatchLink) {
    return null;
  }

  const patterns = (source.watchLinkTextPatterns || []).map((pattern) =>
    normalizeComparableTitle(pattern)
  );
  const candidates = Array.from(
    document.querySelectorAll("a[href], button, [role='button'], [data-href], [data-url]")
  );
  for (const element of candidates) {
    const text = normalizeComparableTitle(
      [
        element.textContent || "",
        element.getAttribute("title") || "",
        element.getAttribute("aria-label") || ""
      ].join(" ")
    );
    if (!text || !patterns.some((pattern) => pattern && text.includes(pattern))) {
      continue;
    }
    const url = elementUrl(source, element, pageUrl);
    if (url) {
      return url;
    }
  }

  return null;
}

function elementUrl(source: SourceConfig, element: Element | null, pageUrl: string): string | null {
  const rawUrl =
    readAttribute(element, "href") ||
    readAttribute(element, "data-href") ||
    readAttribute(element, "data-url");
  return absoluteUrl(source.baseUrl, pageUrl, rawUrl);
}

function readText(element: Element | null): string | null {
  return stripHtml(element?.textContent || "").trim() || null;
}

function selectorExists(html: string, selector: string): boolean {
  if (!selector.trim()) {
    return false;
  }
  try {
    const document = new DOMParser().parseFromString(html, "text/html");
    return document.querySelector(selector) !== null;
  } catch {
    return false;
  }
}

function detectSelectorCandidates(document: Document): SelectorCandidate[] {
  const candidates: SelectorCandidate[] = [];
  const addCandidates = (
    selectorType: SelectorCandidate["selectorType"],
    selectors: string[]
  ) => {
    for (const selector of selectors) {
      try {
        const matches = Array.from(document.querySelectorAll(selector));
        if (matches.length === 0) {
          continue;
        }
        candidates.push({
          selectorType,
          selector,
          matchCount: matches.length,
          sample: stripHtml(matches[0]?.textContent || "").slice(0, 80) || null
        });
      } catch {
        // Ignore invalid browser selectors from the candidate list.
      }
    }
  };

  addCandidates("result", commonResultSelectors);
  addCandidates("title", commonTitleSelectors);
  addCandidates("poster", commonPosterSelectors);

  return candidates
    .sort((left, right) => right.matchCount - left.matchCount)
    .slice(0, 12);
}

function readAttribute(element: Element | null, attribute: string): string | null {
  return element?.getAttribute(attribute)?.trim() || null;
}

function absoluteUrl(baseUrl: string, pageUrl: string, value: string | null): string | null {
  if (!value || value.startsWith("javascript:") || value.startsWith("mailto:")) {
    return null;
  }
  try {
    return new URL(value, pageUrl || baseUrl).toString();
  } catch {
    return null;
  }
}

function browserSourceErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message === "No results or selector not found.") {
    return "No results or selector not found after retry.";
  }
  if (error instanceof TypeError) {
    return "Browser preview could not fetch this source. Run the Tauri desktop app for unrestricted source searching.";
  }
  return error instanceof Error ? error.message : String(error);
}

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function readStorage<T>(key: string, fallback: T): T {
  try {
    const value = localStorage.getItem(key);
    return value ? (JSON.parse(value) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeStorage<T>(key: string, value: T): void {
  localStorage.setItem(key, JSON.stringify(value));
}

function addDeletedDefaultSourceId(defaultSourceId: string): void {
  const ids = new Set(readStorage<string[]>(storageKeys.deletedDefaultSources, []));
  ids.add(defaultSourceId);
  writeStorage(storageKeys.deletedDefaultSources, Array.from(ids));
}
