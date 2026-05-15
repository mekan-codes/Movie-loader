import {
  AlertTriangle,
  Check,
  CircleSlash,
  Clock3,
  ExternalLink,
  Loader2,
  Play,
  Search,
  Star
} from "lucide-react";
import { useState } from "react";
import type {
  Favorite,
  SearchResult,
  SourceConfig,
  SourceSearchOutcome,
  SourceSearchStatus
} from "../types";
import { cx } from "../utils";

interface SearchViewProps {
  query: string;
  onQueryChange: (query: string) => void;
  onSearch: (query?: string) => void;
  isSearching: boolean;
  sources: SourceConfig[];
  builtInSources: Array<{ id: string; name: string; baseUrl: string }>;
  selectedSourceIds: string[];
  onSelectedSourceIdsChange: (ids: string[]) => void;
  recentSearches: string[];
  outcomes: SourceSearchOutcome[];
  favorites: Favorite[];
  onOpenResult: (result: SearchResult) => void;
  onOpenExternalResult: (result: SearchResult) => void;
  onOpenPrimaryWebResults: () => void;
  onRetrySource: (sourceId: string) => void;
  showBulkOpen: boolean;
  onToggleFavorite: (result: SearchResult) => void;
}

export function SearchView({
  query,
  onQueryChange,
  onSearch,
  isSearching,
  sources,
  builtInSources,
  selectedSourceIds,
  onSelectedSourceIdsChange,
  recentSearches,
  outcomes,
  favorites,
  onOpenResult,
  onOpenExternalResult,
  onOpenPrimaryWebResults,
  onRetrySource,
  showBulkOpen,
  onToggleFavorite
}: SearchViewProps) {
  const [expandedSources, setExpandedSources] = useState<string[]>([]);
  const enabledSources = sources.filter(
    (source) => source.enabled && !source.hidden && !source.isDeleted
  );
  const availableSourceCount = enabledSources.length + builtInSources.length;
  const selectedCount = selectedSourceIds.length;
  const totalResults = outcomes.reduce((sum, outcome) => sum + outcome.results.length, 0);
  const primaryWebResultCount = outcomes
    .flatMap((outcome) => outcome.results)
    .filter((result) => result.openMode === "webview")
    .filter((result) => result.rawData?.primary !== "false").length;

  const toggleSource = (sourceId: string) => {
    if (selectedSourceIds.includes(sourceId)) {
      onSelectedSourceIdsChange(selectedSourceIds.filter((id) => id !== sourceId));
    } else {
      onSelectedSourceIdsChange([...selectedSourceIds, sourceId]);
    }
  };

  return (
    <section className="view search-view">
      <header className="view-header">
        <div>
          <h1>Search</h1>
          <p>
            {availableSourceCount} enabled source{availableSourceCount === 1 ? "" : "s"}
          </p>
        </div>
        <div className="metric-strip">
          {showBulkOpen && primaryWebResultCount > 1 && (
            <button className="secondary-button" type="button" onClick={onOpenPrimaryWebResults}>
              <ExternalLink size={17} />
              <span>Open web searches</span>
            </button>
          )}
          <div>
            <span>{outcomes.length}</span>
            <small>Sources checked</small>
          </div>
          <div>
            <span>{totalResults}</span>
            <small>Results</small>
          </div>
        </div>
      </header>

      <form
        className="search-bar"
        onSubmit={(event) => {
          event.preventDefault();
          onSearch();
        }}
      >
        <Search size={20} />
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Movie or show title"
          aria-label="Movie or show title"
        />
        <button className="primary-button" type="submit" disabled={isSearching || !query.trim()}>
          {isSearching ? <Loader2 className="spin" size={18} /> : <Search size={18} />}
          <span>{isSearching ? "Searching" : "Search"}</span>
        </button>
      </form>

      <div className="source-filter-row">
        <button
          className={cx("chip", selectedCount === 0 && "is-selected")}
          type="button"
          onClick={() => onSelectedSourceIdsChange([])}
        >
          All sources
        </button>
        {enabledSources.map((source) => (
          <button
            className={cx("chip", selectedSourceIds.includes(source.id) && "is-selected")}
            key={source.id}
            type="button"
            onClick={() => toggleSource(source.id)}
            title={source.baseUrl}
          >
            {source.name}
          </button>
        ))}
        {builtInSources.map((source) => (
          <button
            className={cx("chip", selectedSourceIds.includes(source.id) && "is-selected")}
            key={source.id}
            type="button"
            onClick={() => toggleSource(source.id)}
            title={source.baseUrl}
          >
            {source.name}
          </button>
        ))}
      </div>

      {recentSearches.length > 0 && (
        <div className="recent-row">
          <span>Recent</span>
          {recentSearches.map((recent) => (
            <button key={recent} type="button" onClick={() => onSearch(recent)}>
              {recent}
            </button>
          ))}
        </div>
      )}

      {availableSourceCount === 0 && (
        <div className="empty-panel">
          <CircleSlash size={20} />
          <strong>No enabled sources</strong>
          <span>Add or enable a source.</span>
        </div>
      )}

      <div className="results-stack">
        {outcomes.map((outcome) => {
          const isExpanded = expandedSources.includes(outcome.sourceId);
          const isAmbiguous = outcome.results.some(
            (result) => result.rawData?.resolution === "ambiguous"
          );
          const displayedResults = isExpanded
            ? outcome.results
            : isAmbiguous
              ? outcome.results.slice(0, 5)
              : outcome.results.slice(0, 1);
          const hiddenCount = Math.max(0, outcome.results.length - displayedResults.length);

          return (
            <section className="source-results" key={outcome.sourceId}>
              <div className="source-results-header">
                <div>
                  <strong>{outcome.sourceName}</strong>
                  <span>
                    {outcome.status === "found" || outcome.status === "ready"
                      ? `${outcome.results.length} result${
                          outcome.results.length === 1 ? "" : "s"
                        } found. ${outcome.message || ""}`
                      : outcome.message}
                  </span>
                </div>
                <div className="source-results-actions">
                  {canRetry(outcome.status) && (
                    <button
                      className="secondary-button compact"
                      type="button"
                      onClick={() => onRetrySource(outcome.sourceId)}
                      disabled={isSearching}
                    >
                      <Loader2 size={15} />
                      <span>Retry</span>
                    </button>
                  )}
                  <StatusBadge status={outcome.status} elapsedMs={outcome.elapsedMs} />
                </div>
              </div>

              {outcome.results.length > 0 ? (
                <>
                  <div className="result-grid">
                    {displayedResults.map((result) => (
                      <ResultCard
                        key={`${result.sourceId}-${result.url}`}
                        result={result}
                        isFavorite={favorites.some((favorite) => favorite.url === result.url)}
                        onOpen={() => onOpenResult(result)}
                        onOpenExternal={() => onOpenExternalResult(result)}
                        onOpenAlternative={(alternative) => onOpenResult(alternative)}
                        onToggleFavorite={() => onToggleFavorite(result)}
                      />
                    ))}
                  </div>
                  {hiddenCount > 0 && (
                    <div className="source-more-row">
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={() =>
                          setExpandedSources((current) =>
                            current.includes(outcome.sourceId)
                              ? current.filter((id) => id !== outcome.sourceId)
                              : [...current, outcome.sourceId]
                          )
                        }
                      >
                        <span>
                          {isExpanded
                            ? "Show best only"
                            : `Show ${hiddenCount} more from this source`}
                        </span>
                      </button>
                    </div>
                  )}
                </>
              ) : (
                <div className="source-empty">
                  {outcome.status === "searching" ||
                  outcome.status === "loading" ||
                  outcome.status === "parsing" ? (
                    <Loader2 className="spin" size={18} />
                  ) : (
                    <CircleSlash size={18} />
                  )}
                  <span>{emptyCopy(outcome.status)}</span>
                </div>
              )}
            </section>
          );
        })}
      </div>
    </section>
  );
}

function ResultCard({
  result,
  isFavorite,
  onOpen,
  onOpenExternal,
  onOpenAlternative,
  onToggleFavorite
}: {
  result: SearchResult;
  isFavorite: boolean;
  onOpen: () => void;
  onOpenExternal: () => void;
  onOpenAlternative: (result: SearchResult) => void;
  onToggleFavorite: () => void;
}) {
  const alternatives = parseAlternatives(result);
  const isProviderCard = result.rawData?.resultKind === "provider";
  const resolutionLabel = resultLabel(result, isProviderCard);

  return (
    <article className="result-card">
      <div className="poster-slot">
        {result.posterUrl ? (
          <img alt="" src={result.posterUrl} loading="lazy" />
        ) : (
          <span>{result.title.slice(0, 1).toUpperCase()}</span>
        )}
      </div>
      <div className="result-body">
        <div className="result-title-row">
          <h3>{result.title}</h3>
          {result.year && <span className="year-pill">{result.year}</span>}
        </div>
        <div className="result-meta">
          <span>{result.sourceName}</span>
          <span>{result.openMode === "webview" ? "In-app viewer" : "Direct video"}</span>
          <span>{resolutionLabel}</span>
          {!isProviderCard && <span>{Math.round(result.confidence)}% match</span>}
        </div>
        {result.description && <p>{result.description}</p>}
        <div className="card-actions">
          <button type="button" className="primary-button compact" onClick={onOpen}>
            <Play size={16} />
            <span>{isProviderCard ? "Open search" : result.openMode === "webview" ? "Open page" : "Play"}</span>
          </button>
          {result.openMode === "webview" && (
            <button
              type="button"
              className="icon-button"
              onClick={onOpenExternal}
              title="Open in system browser"
            >
              <ExternalLink size={16} />
            </button>
          )}
          <button
            className={cx("icon-button", isFavorite && "is-on")}
            type="button"
            onClick={onToggleFavorite}
            title={isFavorite ? "Remove favorite" : "Save favorite"}
          >
            <Star size={17} fill={isFavorite ? "currentColor" : "none"} />
          </button>
        </div>
        {alternatives.length > 0 && (
          <details className="more-options">
            <summary>More options</summary>
            <div className="more-options-list">
              {alternatives.map((alternative, index) => (
                <button
                  className="ghost-button"
                  type="button"
                  key={`${alternative.url}-${index}`}
                  onClick={() =>
                    onOpenAlternative({
                      ...result,
                      id: `${result.id}-alt-${index}`,
                      title: alternative.label,
                      url: alternative.url,
                      rawData: {
                        ...result.rawData,
                        copyQuery: alternative.copyQuery || "",
                        searchOption: alternative.label
                      }
                    })
                  }
                >
                  <ExternalLink size={15} />
                  <span>{alternative.label}</span>
                </button>
              ))}
            </div>
          </details>
        )}
      </div>
    </article>
  );
}

function resultLabel(result: SearchResult, isProviderCard: boolean): string {
  if (isProviderCard) {
    return "Search fallback";
  }
  if (result.rawData?.resolution === "ambiguous") {
    return "Possible match";
  }
  if (result.rawData?.openedVia === "watchButtonSelector") {
    return "Ready to watch";
  }
  if (result.playableUrl) {
    return "Ready to play";
  }
  return "Movie page found";
}

function StatusBadge({
  status,
  elapsedMs
}: {
  status: SourceSearchStatus;
  elapsedMs: number;
}) {
  const icon =
    status === "searching" ? (
      <Loader2 className="spin" size={15} />
    ) : status === "loading" || status === "parsing" ? (
      <Loader2 className="spin" size={15} />
    ) : status === "found" || status === "ready" ? (
      <Check size={15} />
    ) : status === "timed_out" ? (
      <Clock3 size={15} />
    ) : status === "error" || status === "unsupported" ? (
      <AlertTriangle size={15} />
    ) : (
      <CircleSlash size={15} />
    );

  return (
    <span className={cx("status-badge", `status-${status}`)}>
      {icon}
      {labelForStatus(status)}
      {elapsedMs > 0 && <em>{elapsedMs}ms</em>}
    </span>
  );
}

function labelForStatus(status: SourceSearchStatus): string {
  switch (status) {
    case "searching":
      return "Searching";
    case "loading":
      return "Loading page";
    case "parsing":
      return "Parsing";
    case "found":
      return "Found";
    case "ready":
      return "Available search";
    case "not_found":
      return "No results";
    case "timed_out":
      return "Timed out";
    case "unsupported":
      return "Unsupported";
    case "error":
    default:
      return "Error";
  }
}

function emptyCopy(status: SourceSearchStatus): string {
  switch (status) {
    case "searching":
      return "Checking source";
    case "loading":
      return "Loading page";
    case "parsing":
      return "Parsing results";
    case "unsupported":
      return "Unsupported source";
    case "timed_out":
      return "Timed out";
    case "error":
      return "Error";
    default:
      return "No results";
  }
}

function canRetry(status: SourceSearchStatus): boolean {
  return status === "error" || status === "timed_out" || status === "not_found" || status === "unsupported";
}

function parseAlternatives(result: SearchResult): Array<{
  label: string;
  url: string;
  description?: string;
  copyQuery?: string;
}> {
  const raw = result.rawData?.alternatives;
  if (!raw) {
    return [];
  }
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter((item): item is Record<string, string> => Boolean(item) && typeof item === "object")
      .flatMap((item, index) => {
        if (typeof item.url !== "string" || !item.url) {
          return [];
        }
        return [
          {
            label:
              typeof item.label === "string" && item.label
                ? readableOptionLabel(item.label, index)
                : `Option ${index + 2}`,
            url: item.url,
            description: item.description,
            copyQuery: item.copyQuery
          }
        ];
      });
  } catch {
    return [];
  }
}

function readableOptionLabel(value: string, index: number): string {
  if (value === "manual") {
    return "Open homepage";
  }
  if (value.startsWith("alternate")) {
    return `Alternative ${index + 2}`;
  }
  return value;
}
