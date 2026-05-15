import type { SourceConfig } from "./types";
import { createId } from "./utils";

const builtInHosts = [
  "youtube.com",
  "youtu.be",
  "therokuchannel.roku.com",
  "watch.sling.com",
  "fawesome.tv",
  "nfb.ca"
];

const searchParamNames = ["q", "query", "search", "s", "term", "keyword", "keywords", "text"];

export interface InferredSource {
  source: SourceConfig;
  note: string;
}

export function inferSourceFromUrl(input: string): InferredSource {
  const parsed = parseUrl(input);
  const host = parsed.hostname.replace(/^www\./, "");
  const builtIn = builtInHosts.find((candidate) => host === candidate || host.endsWith(`.${candidate}`));

  const searchUrl = inferSearchUrl(parsed);
  const source: SourceConfig = {
    id: createId(slugify(host) || "source"),
    name: readableHost(host),
    enabled: true,
    isDefault: false,
    userModified: false,
    hidden: false,
    sourceKind: "web",
    sourceType: "search",
    sourceOpenBehavior: "webview",
    resultOpenBehavior: "result_page",
    ambiguousQueryBehavior: "show_choices",
    baseUrl: parsed.origin,
    searchUrl,
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
      "watch now",
      "play",
      "start watching",
      "смотреть",
      "смотреть онлайн"
    ],
    episodeSelector: "",
    seasonSelector: "",
    playerSelector: "video, iframe",
    autoOpenFirstWatchLink: false,
    autoOpenBestMatch: true,
    autoOpenWatchButton: true,
    maxWatchResolveSteps: 2,
    exactMatchThreshold: 85,
    requiresJavaScript: true,
    headers: {}
  };

  return {
    source,
    note:
      builtIn
        ? "Added as a web source. This site is also available as a built-in source."
        : "Added as a web source. Searches open in the in-app viewer without selector setup."
  };
}

function parseUrl(input: string): URL {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("Enter a website URL first.");
  }
  try {
    return new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
  } catch {
    throw new Error("Enter a valid website URL.");
  }
}

function inferSearchUrl(url: URL): string {
  if (url.href.includes("{query}")) {
    return url.href;
  }

  const knownTemplate = knownSearchTemplate(url);
  if (knownTemplate) {
    return knownTemplate;
  }

  const withQuery = new URL(url.href);
  const existingSearchParam = searchParamNames.find((name) => withQuery.searchParams.has(name));
  if (existingSearchParam) {
    withQuery.searchParams.set(existingSearchParam, "{query}");
    return decodeQueryToken(withQuery.toString());
  }

  return `${url.origin}/search?q={query}`;
}

function knownSearchTemplate(url: URL): string | null {
  const host = url.hostname.replace(/^www\./, "");
  if (host === "youtube.com" || host.endsWith(".youtube.com") || host === "youtu.be") {
    return "https://www.youtube.com/results?search_query={query}";
  }
  if (host === "watch.plex.tv" || host.endsWith(".watch.plex.tv")) {
    return "https://watch.plex.tv/search?query={query}";
  }
  if (host === "plex.tv" || host.endsWith(".plex.tv")) {
    return "https://watch.plex.tv/search?query={query}";
  }
  if (host === "tubitv.com" || host.endsWith(".tubitv.com")) {
    return "https://tubitv.com/search/{query}";
  }
  if (host === "archive.org" || host.endsWith(".archive.org")) {
    return "https://archive.org/search?query={query}";
  }
  return null;
}

function decodeQueryToken(value: string): string {
  return value.replace(/%7Bquery%7D/gi, "{query}");
}

function readableHost(host: string): string {
  return host
    .replace(/^www\./, "")
    .split(".")
    .filter(Boolean)
    .slice(0, -1)
    .join(" ")
    .replace(/(^|\s)\S/g, (match) => match.toUpperCase())
    .trim() || host;
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
