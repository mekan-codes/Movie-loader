import type { SourceConfig } from "../types";
import { createId } from "../utils";

const defaultTiming = {
  loadDelayMs: 1500,
  maxRetries: 2,
  requestTimeoutMs: 15000,
  resolveDelayMs: 1500
};

export const removedDefaultSourceIds = new Set([
  "plex",
  "tubi",
  "pluto-tv",
  "filmzie",
  "xumo-play",
  "filmrise",
  "arte-tv"
]);

export const defaultSources: SourceConfig[] = [
  createDefaultSource({
    defaultSourceId: "roku-channel",
    name: "The Roku Channel",
    baseUrl: "https://therokuchannel.roku.com",
    searchUrl: "https://therokuchannel.roku.com/search/{query}",
    note: "Free ad-supported catalog; region and account behavior can vary."
  }),
  createDefaultSource({
    defaultSourceId: "sling-freestream",
    name: "Sling Freestream",
    baseUrl: "https://watch.sling.com",
    searchUrl: "https://watch.sling.com/search?query={query}",
    note: "Free streaming area; region and WebView behavior can vary."
  }),
  createDefaultSource({
    defaultSourceId: "fawesome",
    name: "Fawesome",
    baseUrl: "https://fawesome.tv",
    searchUrl: "https://fawesome.tv/search?query={query}",
    note: "Free ad-supported catalog; WebView fallback may be needed."
  }),
  createDefaultSource({
    defaultSourceId: "youtube-movies-tv",
    name: "YouTube Movies & TV",
    baseUrl: "https://www.youtube.com/feed/storefront",
    searchUrl: "https://www.youtube.com/results?search_query={query}",
    note: "May include rentals, purchases, clips, and region-dependent free titles."
  }),
  createDefaultSource({
    defaultSourceId: "nfb-ca",
    name: "NFB.ca",
    baseUrl: "https://www.nfb.ca",
    searchUrl: "https://www.nfb.ca/search/?q={query}",
    note: "National Film Board of Canada catalog; some items may be region-limited."
  })
];

export const sampleSources = defaultSources;

export function createEmptySource(): SourceConfig {
  return {
    id: createId("source"),
    name: "",
    enabled: true,
    isDefault: false,
    userModified: false,
    hidden: false,
    isDeleted: false,
    deletedAt: null,
    sourceKind: "web",
    sourceType: "search",
    sourceOpenBehavior: "webview",
    resultOpenBehavior: "result_page",
    ambiguousQueryBehavior: "show_choices",
    parserMode: "hybrid",
    baseUrl: "",
    searchUrl: "",
    method: "GET",
    resultSelector: "",
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
    watchLinkTextPatterns: defaultWatchPatterns(),
    episodeSelector: "",
    seasonSelector: "",
    playerSelector: "video, iframe",
    autoResolveWatchPage: true,
    autoOpenFirstWatchLink: false,
    autoOpenBestMatch: true,
    autoOpenWatchButton: true,
    maxWatchResolveSteps: 2,
    maxResolveSteps: 2,
    exactMatchThreshold: 85,
    requiresJavaScript: true,
    headers: {},
    ...defaultTiming
  };
}

function createDefaultSource(input: {
  defaultSourceId: string;
  name: string;
  baseUrl: string;
  searchUrl: string;
  note: string;
}): SourceConfig {
  return {
    id: `default-${input.defaultSourceId}`,
    defaultSourceId: input.defaultSourceId,
    isDefault: true,
    userModified: false,
    hidden: false,
    isDeleted: false,
    deletedAt: null,
    name: input.name,
    enabled: true,
    note: input.note,
    sourceKind: "web",
    sourceType: "search",
    sourceOpenBehavior: "webview",
    resultOpenBehavior: "result_page",
    ambiguousQueryBehavior: "show_choices",
    parserMode: "hybrid",
    baseUrl: input.baseUrl,
    searchUrl: input.searchUrl,
    method: "GET",
    resultSelector: "",
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
    watchLinkTextPatterns: defaultWatchPatterns(),
    episodeSelector: "",
    seasonSelector: "",
    playerSelector: "video, iframe",
    autoResolveWatchPage: true,
    autoOpenFirstWatchLink: false,
    autoOpenBestMatch: true,
    autoOpenWatchButton: true,
    maxWatchResolveSteps: 2,
    maxResolveSteps: 2,
    exactMatchThreshold: 85,
    requiresJavaScript: true,
    headers: {},
    ...defaultTiming
  };
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
