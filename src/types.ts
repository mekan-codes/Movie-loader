export type ViewKey =
  | "search"
  | "sources"
  | "viewer"
  | "favorites"
  | "history"
  | "downloads"
  | "settings";

export type ThemeMode = "system" | "light" | "dark";
export type PlayerMode = "native" | "webview" | "ask";
export type SearchBehavior = "enabled" | "lastSelected";
export type ResultOpenMode = "native" | "webview";
export type SourceKind = "web" | "direct";
export type OpenBehavior = "inApp" | "external";
export type SourceType = "search" | "directPage" | "webviewOnly";
export type SourceOpenBehavior = "webview" | "nativeThenWebview";
export type ResultPageBehavior = "result_page" | "search_page";
export type AmbiguousQueryBehavior = "show_choices" | "open_search_page";
export type ParserMode = "static" | "webview" | "hybrid" | "fallbackOnly";
export type ResultQuality = "excellent" | "good" | "medium" | "weak" | "failed";

export interface SourceDebugInfo {
  generatedSearchUrl?: string | null;
  finalLoadedUrl?: string | null;
  parserModeUsed?: ParserMode | string | null;
  staticFetchWorked?: boolean | null;
  staticParseWorked?: boolean | null;
  webViewParseWorked?: boolean | null;
  htmlLength?: number | null;
  resultContainerCount?: number | null;
  candidateTitles?: string[];
  candidateLinks?: string[];
  bestScore?: number | null;
  whyAutoOpenFailed?: string | null;
  javascriptProbablyRequired?: boolean | null;
  browserPreviewLimited?: boolean | null;
  timeoutOrError?: string | null;
  finalAction?: "exact page" | "show choices" | "search fallback" | "failed" | string | null;
}

export interface SourceConfig {
  id: string;
  name: string;
  enabled: boolean;
  defaultSourceId?: string | null;
  isDefault?: boolean;
  userModified?: boolean;
  hidden?: boolean;
  isDeleted?: boolean;
  deletedAt?: string | null;
  note?: string | null;
  sourceType?: SourceType;
  sourceOpenBehavior?: SourceOpenBehavior;
  resultOpenBehavior?: ResultPageBehavior;
  ambiguousQueryBehavior?: AmbiguousQueryBehavior;
  sourceKind?: SourceKind;
  parserMode?: ParserMode;
  baseUrl: string;
  searchUrl: string;
  method: "GET" | "POST" | string;
  resultSelector: string;
  loadDelayMs?: number;
  maxRetries?: number;
  requestTimeoutMs?: number;
  waitForSelector?: string | null;
  titleSelector?: string | null;
  posterSelector?: string | null;
  posterAttribute?: string | null;
  linkSelector?: string | null;
  linkAttribute?: string | null;
  yearSelector?: string | null;
  descriptionSelector?: string | null;
  videoSelector?: string | null;
  videoAttribute?: string | null;
  iframeSelector?: string | null;
  iframeAttribute?: string | null;
  subtitleSelector?: string | null;
  subtitleAttribute?: string | null;
  subtitleLanguageAttribute?: string | null;
  audioLanguageSelector?: string | null;
  downloadSelector?: string | null;
  downloadAttribute?: string | null;
  watchButtonSelector?: string | null;
  watchLinkTextPatterns?: string[];
  episodeSelector?: string | null;
  seasonSelector?: string | null;
  playerSelector?: string | null;
  autoResolveWatchPage?: boolean;
  autoOpenFirstWatchLink?: boolean;
  autoOpenBestMatch?: boolean;
  autoOpenWatchButton?: boolean;
  maxWatchResolveSteps?: number;
  maxResolveSteps?: number;
  resolveDelayMs?: number;
  exactMatchThreshold?: number;
  requiresJavaScript: boolean;
  headers: Record<string, string>;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface SearchResult {
  id: string;
  sourceId: string;
  sourceName: string;
  title: string;
  url: string;
  openMode?: ResultOpenMode;
  playableUrl?: string | null;
  posterUrl?: string | null;
  year?: string | null;
  description?: string | null;
  confidence: number;
  quality?: ResultQuality;
  reason?: string | null;
  sourceReliability?: number;
  debugInfo?: SourceDebugInfo | null;
  rawData?: Record<string, string>;
}

export type SourceSearchStatus =
  | "searching"
  | "loading"
  | "parsing"
  | "ready"
  | "found"
  | "not_found"
  | "error"
  | "timed_out"
  | "unsupported";

export interface SourceSearchOutcome {
  sourceId: string;
  sourceName: string;
  status: SourceSearchStatus;
  message?: string | null;
  elapsedMs: number;
  results: SearchResult[];
  debugInfo?: SourceDebugInfo | null;
}

export interface SourceTestResult {
  ok: boolean;
  message: string;
  resultCount: number;
  elapsedMs: number;
  finalSearchUrl?: string | null;
  rawStatus?: string | null;
  selectorMatchCount?: number;
  previewResults?: SourcePreviewResult[];
  fallbackUsed?: boolean;
  detectedSelectors?: SelectorCandidate[];
  bestMatch?: SourcePreviewResult | null;
  finalOpenUrl?: string | null;
  querySpecificity?: string | null;
  ambiguous?: boolean;
  debugInfo?: SourceDebugInfo | null;
}

export interface SourcePreviewResult {
  title: string;
  url: string;
  year?: string | null;
  score?: number;
  confidenceReason?: string | null;
}

export interface SelectorCandidate {
  selectorType: "result" | "title" | "poster";
  selector: string;
  matchCount: number;
  sample?: string | null;
}

export interface Favorite {
  id: string;
  title: string;
  sourceName: string;
  url: string;
  openMode?: ResultOpenMode;
  playableUrl?: string | null;
  posterUrl?: string | null;
  createdAt?: string | null;
}

export interface HistoryItem {
  id: string;
  title: string;
  sourceName: string;
  url: string;
  openMode?: ResultOpenMode;
  playableUrl?: string | null;
  posterUrl?: string | null;
  lastOpenedAt?: string | null;
  playbackPositionSeconds: number;
  durationSeconds: number;
}

export interface DownloadItem {
  id: string;
  title: string;
  sourceName: string;
  url: string;
  filePath?: string | null;
  status: "queued" | "downloading" | "paused" | "cancelled" | "completed" | "error";
  progress: number;
  createdAt: string;
}

export interface AppSettings {
  theme: ThemeMode;
  defaultPlayerMode: PlayerMode;
  defaultSearchBehavior: SearchBehavior;
  defaultDownloadFolder: string;
  webOpenDelaySeconds: number;
  openBehavior: OpenBehavior;
}

export interface AppExport {
  version: 1;
  exportedAt: string;
  sources: SourceConfig[];
  favorites: Favorite[];
  history: HistoryItem[];
  settings: AppSettings;
}
