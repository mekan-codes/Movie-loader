import type {
  AppSettings,
  Favorite,
  HistoryItem,
  SearchResult,
  SourceConfig
} from "./types";

export const defaultSettings: AppSettings = {
  theme: "system",
  defaultPlayerMode: "webview",
  defaultSearchBehavior: "enabled",
  defaultDownloadFolder: "",
  webOpenDelaySeconds: 5,
  openBehavior: "inApp"
};

export function createId(prefix = "item"): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 9)}`;
}

export function stableId(value: string, prefix = "item"): string {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return `${prefix}-${(hash >>> 0).toString(16)}`;
}

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function formatDate(value?: string | null): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(date);
}

export function normalizeSource(source: SourceConfig): SourceConfig {
  const maxResolveSteps = clampNumber(
    source.maxResolveSteps ?? source.maxWatchResolveSteps,
    0,
    5,
    2
  );

  return {
    ...source,
    id: source.id.trim() || createId("source"),
    name: source.name.trim(),
    defaultSourceId: cleanOptional(source.defaultSourceId),
    isDefault: Boolean(source.isDefault),
    userModified: Boolean(source.userModified),
    hidden: Boolean(source.hidden),
    isDeleted: Boolean(source.isDeleted),
    deletedAt: source.isDeleted ? cleanOptional(source.deletedAt) : null,
    note: cleanOptional(source.note),
    sourceType: source.sourceType || "search",
    sourceOpenBehavior: source.sourceOpenBehavior || "webview",
    resultOpenBehavior: source.resultOpenBehavior || "result_page",
    ambiguousQueryBehavior: source.ambiguousQueryBehavior || "show_choices",
    sourceKind: source.sourceKind === "direct" ? "direct" : "web",
    parserMode: normalizeParserMode(source.parserMode, source.sourceType, source.requiresJavaScript),
    baseUrl: source.baseUrl.trim(),
    searchUrl: source.searchUrl.trim(),
    method: (source.method || "GET").trim().toUpperCase(),
    resultSelector: source.resultSelector.trim(),
    loadDelayMs: clampNumber(source.loadDelayMs, 0, 10000, 1500),
    maxRetries: clampNumber(source.maxRetries, 0, 5, 2),
    requestTimeoutMs: clampNumber(source.requestTimeoutMs, 3000, 60000, 15000),
    waitForSelector: cleanOptional(source.waitForSelector),
    titleSelector: cleanOptional(source.titleSelector),
    posterSelector: cleanOptional(source.posterSelector),
    posterAttribute: cleanOptional(source.posterAttribute) || "src",
    linkSelector: cleanOptional(source.linkSelector),
    linkAttribute: cleanOptional(source.linkAttribute) || "href",
    yearSelector: cleanOptional(source.yearSelector),
    descriptionSelector: cleanOptional(source.descriptionSelector),
    videoSelector: cleanOptional(source.videoSelector),
    videoAttribute: cleanOptional(source.videoAttribute) || "src",
    iframeSelector: cleanOptional(source.iframeSelector),
    iframeAttribute: cleanOptional(source.iframeAttribute) || "src",
    subtitleSelector: cleanOptional(source.subtitleSelector),
    subtitleAttribute: cleanOptional(source.subtitleAttribute) || "src",
    subtitleLanguageAttribute: cleanOptional(source.subtitleLanguageAttribute) || "srclang",
    audioLanguageSelector: cleanOptional(source.audioLanguageSelector),
    downloadSelector: cleanOptional(source.downloadSelector),
    downloadAttribute: cleanOptional(source.downloadAttribute) || "href",
    watchButtonSelector: cleanOptional(source.watchButtonSelector),
    watchLinkTextPatterns:
      Array.isArray(source.watchLinkTextPatterns) && source.watchLinkTextPatterns.length > 0
        ? source.watchLinkTextPatterns.map((pattern) => pattern.trim()).filter(Boolean)
        : defaultWatchPatterns(),
    episodeSelector: cleanOptional(source.episodeSelector),
    seasonSelector: cleanOptional(source.seasonSelector),
    playerSelector: cleanOptional(source.playerSelector) || "video, iframe",
    autoResolveWatchPage: source.autoResolveWatchPage !== false,
    autoOpenFirstWatchLink: Boolean(source.autoOpenFirstWatchLink),
    autoOpenBestMatch: source.autoOpenBestMatch !== false,
    autoOpenWatchButton: source.autoOpenWatchButton !== false,
    maxWatchResolveSteps: maxResolveSteps,
    maxResolveSteps,
    resolveDelayMs: clampNumber(source.resolveDelayMs, 0, 10000, 1500),
    exactMatchThreshold: clampNumber(source.exactMatchThreshold, 50, 100, 85),
    headers: source.headers || {}
  };
}

export function resultToFavorite(result: SearchResult): Favorite {
  return {
    id: stableId(result.url, "favorite"),
    title: result.title,
    sourceName: result.sourceName,
    url: result.url,
    openMode: result.openMode,
    playableUrl: result.playableUrl,
    posterUrl: result.posterUrl,
    createdAt: new Date().toISOString()
  };
}

export function resultToHistory(result: SearchResult): HistoryItem {
  return {
    id: stableId(result.url, "history"),
    title: result.title,
    sourceName: result.sourceName,
    url: result.url,
    openMode: result.openMode,
    playableUrl: result.playableUrl,
    posterUrl: result.posterUrl,
    lastOpenedAt: new Date().toISOString(),
    playbackPositionSeconds: 0,
    durationSeconds: 0
  };
}

export function historyToResult(item: HistoryItem): SearchResult {
  return {
    id: item.id,
    sourceId: item.sourceName,
    sourceName: item.sourceName,
    title: item.title,
    url: item.url,
    openMode: item.openMode,
    playableUrl: item.playableUrl,
    posterUrl: item.posterUrl,
    confidence: 100,
    rawData: {}
  };
}

export function favoriteToResult(item: Favorite): SearchResult {
  return {
    id: item.id,
    sourceId: item.sourceName,
    sourceName: item.sourceName,
    title: item.title,
    url: item.url,
    openMode: item.openMode,
    playableUrl: item.playableUrl,
    posterUrl: item.posterUrl,
    confidence: 100,
    rawData: {}
  };
}

export function downloadJson(filename: string, data: unknown): void {
  const blob = new Blob([JSON.stringify(data, null, 2)], {
    type: "application/json"
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

export async function readJsonFile<T>(file: File): Promise<T> {
  const text = await file.text();
  return JSON.parse(text) as T;
}

export function parseHeaders(value: string): Record<string, string> {
  if (!value.trim()) {
    return {};
  }
  const parsed = JSON.parse(value) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("Headers must be a JSON object.");
  }
  return Object.fromEntries(
    Object.entries(parsed as Record<string, unknown>)
      .filter(([, headerValue]) => typeof headerValue === "string")
      .map(([key, headerValue]) => [key, headerValue as string])
  );
}

export function stringifyHeaders(headers: Record<string, string>): string {
  if (!headers || Object.keys(headers).length === 0) {
    return "";
  }
  return JSON.stringify(headers, null, 2);
}

export function sourceJsonExportName(): string {
  return `cinefinder-sources-${new Date().toISOString().slice(0, 10)}.json`;
}

function cleanOptional(value?: string | null): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function clampNumber(
  value: number | null | undefined,
  min: number,
  max: number,
  fallback: number
): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.round(value as number)));
}

function defaultWatchPatterns(): string[] {
  return [
    "watch full movie",
    "watch online",
    "watch now",
    "play",
    "start watching",
    "смотреть",
    "смотреть онлайн"
  ];
}

function normalizeParserMode(
  value: SourceConfig["parserMode"],
  sourceType: SourceConfig["sourceType"],
  requiresJavaScript: boolean
): NonNullable<SourceConfig["parserMode"]> {
  if (
    value === "static" ||
    value === "webview" ||
    value === "hybrid" ||
    value === "fallbackOnly"
  ) {
    return value;
  }
  if (sourceType === "webviewOnly") {
    return "fallbackOnly";
  }
  return requiresJavaScript ? "hybrid" : "static";
}
