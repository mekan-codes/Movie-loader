import {
  Check,
  Copy,
  Download,
  Film,
  Globe2,
  Pencil,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Sparkles,
  TestTube2,
  Trash2,
  Undo2,
  Upload,
  Wand2,
  X
} from "lucide-react";
import { useMemo, useState } from "react";
import { createEmptySource } from "../data/samples";
import { inferSourceFromUrl } from "../sourceInference";
import type {
  SelectorCandidate,
  ResultPageBehavior,
  AmbiguousQueryBehavior,
  ParserMode,
  SourceConfig,
  SourceKind,
  SourceOpenBehavior,
  SourceTestResult,
  SourceType
} from "../types";
import {
  cx,
  downloadJson,
  normalizeSource,
  parseHeaders,
  readJsonFile,
  sourceJsonExportName,
  stringifyHeaders
} from "../utils";

interface SourcesViewProps {
  sources: SourceConfig[];
  onSave: (source: SourceConfig) => Promise<void>;
  onDelete: (sourceId: string) => Promise<void>;
  onRestore: (sourceId: string) => Promise<void>;
  onPermanentDelete: (sourceId: string) => Promise<void>;
  onEmptyTrash: () => Promise<void>;
  onTest: (source: SourceConfig, query?: string) => Promise<SourceTestResult>;
  onImport: (sources: SourceConfig[]) => Promise<void>;
  onDuplicate: (source: SourceConfig) => Promise<void>;
  onResetSource: (sourceId: string) => Promise<void>;
  onRestoreDefaults: () => Promise<void>;
  onResetAllDefaults: () => Promise<void>;
}

const resultSelectorCandidates = [
  "a[href]",
  "article",
  ".movie",
  ".movie-card",
  ".film",
  ".item",
  ".card",
  ".poster",
  ".video",
  ".entry",
  ".result",
  ".ml-item",
  ".flw-item",
  ".film_list-wrap .flw-item",
  ".content .item",
  ".short",
  ".b-content__inline_item"
];

const titleSelectorCandidates = [
  "h1",
  "h2",
  "h3",
  "h4",
  ".title",
  ".name",
  ".movie-title",
  ".film-title",
  ".entry-title",
  ".b-content__inline_item-link",
  ".short-title",
  "[title]"
];

const posterSelectorCandidates = [
  "img",
  ".poster img",
  ".thumb img",
  "picture img",
  "img[data-src]",
  "img[data-original]",
  "img[data-lazy-src]"
];

export function SourcesView({
  sources,
  onSave,
  onDelete,
  onRestore,
  onPermanentDelete,
  onEmptyTrash,
  onTest,
  onImport,
  onDuplicate,
  onResetSource,
  onRestoreDefaults,
  onResetAllDefaults
}: SourcesViewProps) {
  const [draft, setDraft] = useState<SourceConfig>(() => createEmptySource());
  const [headersText, setHeadersText] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<SourceTestResult | null>(null);
  const [diagnosticResults, setDiagnosticResults] = useState<
    Array<{ source: SourceConfig; result: SourceTestResult }>
  >([]);
  const [diagnosticsRunning, setDiagnosticsRunning] = useState(false);
  const [quickUrl, setQuickUrl] = useState("");
  const [testQuery, setTestQuery] = useState("john wick");
  const [undoDraft, setUndoDraft] = useState<SourceConfig>(() => draft);
  const [undoHeadersText, setUndoHeadersText] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [sourceTab, setSourceTab] = useState<"active" | "disabled" | "trash">("active");

  const sortedSources = useMemo(
    () =>
      sources
        .filter((source) =>
          sourceTab === "active"
            ? source.enabled && !source.hidden && !source.isDeleted
            : sourceTab === "disabled"
              ? !source.enabled && !source.hidden && !source.isDeleted
              : Boolean(source.isDeleted)
        )
        .sort((left, right) => {
        if (Boolean(left.hidden) !== Boolean(right.hidden)) {
          return left.hidden ? 1 : -1;
        }
        return left.name.localeCompare(right.name);
      }),
    [sourceTab, sources]
  );
  const activeCount = sources.filter(
    (source) => source.enabled && !source.hidden && !source.isDeleted
  ).length;
  const disabledCount = sources.filter(
    (source) => !source.enabled && !source.hidden && !source.isDeleted
  ).length;
  const trashCount = sources.filter((source) => source.isDeleted).length;
  const draftKind: SourceKind = draft.sourceKind === "direct" ? "direct" : "web";
  const sourceType = draft.sourceType || "search";

  const beginNew = () => {
    const empty = createEmptySource();
    setDraft(empty);
    setHeadersText("");
    setUndoDraft(empty);
    setUndoHeadersText("");
    setEditingId(null);
    setShowAdvanced(false);
    setMessage(null);
    setTestResult(null);
  };

  const beginEdit = (source: SourceConfig) => {
    const normalized = normalizeSource(source);
    setDraft(normalized);
    const nextHeadersText = stringifyHeaders(normalized.headers);
    setHeadersText(nextHeadersText);
    setUndoDraft(normalized);
    setUndoHeadersText(nextHeadersText);
    setEditingId(normalized.id);
    setShowAdvanced(false);
    setMessage(null);
    setTestResult(null);
  };

  const saveDraft = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const source = normalizeForSave(draft, headersText);
      validateDraft(source, showAdvanced || Boolean(headersText.trim()));
      await onSave(source);
      setMessage("Source saved.");
      beginNew();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const testDraft = async (source: SourceConfig = draft) => {
    setTestingId(source.id);
    setMessage(null);
    try {
      const sourceForTest =
        source.id === draft.id ? normalizeForSave(source, headersText) : normalizeSource(source);
      validateDraft(sourceForTest, source.id === draft.id && Boolean(headersText.trim()));
      const result = await onTest(sourceForTest, testQuery);
      setMessage(
        `${result.ok ? "OK" : "Failed"}: ${result.message} (${result.resultCount} result${
          result.resultCount === 1 ? "" : "s"
        }, ${result.elapsedMs}ms)`
      );
      setTestResult(result);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setTestingId(null);
    }
  };

  const importFile = async (file: File) => {
    setBusy(true);
    setMessage(null);
    try {
      const data = await readJsonFile<SourceConfig[] | { sources: SourceConfig[] }>(file);
      const imported = Array.isArray(data) ? data : data.sources;
      if (!Array.isArray(imported)) {
        throw new Error("Import file must contain an array of source configs.");
      }
      await onImport(imported);
      setMessage(`Imported ${imported.length} source${imported.length === 1 ? "" : "s"}.`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const autoFillFromUrl = () => {
    setMessage(null);
    try {
      const inferred = inferSourceFromUrl(quickUrl);
      setUndoDraft(draft);
      setUndoHeadersText(headersText);
      setDraft(inferred.source);
      setHeadersText("");
      setEditingId(null);
      setShowAdvanced(false);
      setMessage(inferred.note);
      setTestResult(null);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  };

  const quickAddFromUrl = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const inferred = inferSourceFromUrl(quickUrl);
      await onSave(normalizeSource(inferred.source));
      setQuickUrl("");
      setMessage(`${inferred.source.name} added. ${inferred.note}`);
      beginNew();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const undoDraftChanges = () => {
    setDraft(undoDraft);
    setHeadersText(undoHeadersText);
    setMessage("Editor changes undone.");
  };

  const updateDraft = <Key extends keyof SourceConfig>(
    key: Key,
    value: SourceConfig[Key]
  ) => {
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const switchEditorMode = (advanced: boolean) => {
    setShowAdvanced(advanced);
    if (!advanced) {
      switchSourceKind("web");
    }
  };

  const switchSourceKind = (sourceKind: SourceKind) => {
    setDraft((current) => ({
      ...current,
      sourceKind,
      method: "GET",
      resultSelector:
        sourceKind === "direct"
          ? current.resultSelector || ".movie-card, .card, .item, article"
          : current.resultSelector || "",
      waitForSelector:
        sourceKind === "direct"
          ? current.waitForSelector || current.resultSelector || ".movie-card, .card, .item, article"
          : current.waitForSelector || "",
      requiresJavaScript: sourceKind === "web" ? true : current.requiresJavaScript,
      parserMode: sourceKind === "direct" ? "static" : "hybrid"
    }));
  };

  const applyTemplate = (template: SourceTemplate) => {
    setUndoDraft(draft);
    setUndoHeadersText(headersText);
    setShowAdvanced(template.advanced);
    setDraft((current) =>
      normalizeSource({
        ...current,
        sourceKind: template.patch.sourceKind ?? current.sourceKind,
        sourceType: template.patch.sourceType ?? current.sourceType,
        sourceOpenBehavior: template.patch.sourceOpenBehavior ?? current.sourceOpenBehavior,
        parserMode: template.patch.parserMode ?? current.parserMode,
        resultOpenBehavior: template.patch.resultOpenBehavior ?? current.resultOpenBehavior,
        ambiguousQueryBehavior:
          template.patch.ambiguousQueryBehavior ?? current.ambiguousQueryBehavior,
        requiresJavaScript: template.patch.requiresJavaScript ?? current.requiresJavaScript,
        autoOpenBestMatch: template.patch.autoOpenBestMatch ?? current.autoOpenBestMatch,
        autoResolveWatchPage:
          template.patch.autoResolveWatchPage ?? current.autoResolveWatchPage,
        autoOpenWatchButton: template.patch.autoOpenWatchButton ?? current.autoOpenWatchButton,
        maxWatchResolveSteps: template.patch.maxWatchResolveSteps ?? current.maxWatchResolveSteps,
        maxResolveSteps: template.patch.maxResolveSteps ?? current.maxResolveSteps,
        resolveDelayMs: template.patch.resolveDelayMs ?? current.resolveDelayMs,
        exactMatchThreshold: template.patch.exactMatchThreshold ?? current.exactMatchThreshold,
        resultSelector: template.patch.resultSelector ?? current.resultSelector,
        titleSelector: template.patch.titleSelector ?? current.titleSelector,
        linkSelector: template.patch.linkSelector ?? current.linkSelector,
        linkAttribute: template.patch.linkAttribute ?? current.linkAttribute,
        posterSelector: template.patch.posterSelector ?? current.posterSelector,
        posterAttribute: template.patch.posterAttribute ?? current.posterAttribute,
        videoSelector: template.patch.videoSelector ?? current.videoSelector,
        iframeSelector: template.patch.iframeSelector ?? current.iframeSelector,
        waitForSelector: template.patch.waitForSelector ?? current.waitForSelector,
        loadDelayMs: template.patch.loadDelayMs ?? current.loadDelayMs,
        maxRetries: template.patch.maxRetries ?? current.maxRetries,
        requestTimeoutMs: template.patch.requestTimeoutMs ?? current.requestTimeoutMs
      })
    );
    setMessage(`${template.label} template applied.`);
  };

  const applyCommonSelectors = () => {
    setUndoDraft(draft);
    setUndoHeadersText(headersText);
    setShowAdvanced(true);
    setDraft((current) =>
      normalizeSource({
        ...current,
        sourceKind: "direct",
        parserMode: "hybrid",
        requiresJavaScript: false,
        resultSelector: ".movie-card, .card, .item, article, a[href]",
        waitForSelector: ".movie-card, .card, .item, article, a[href]",
        titleSelector: ".title, .name, .movie-title, h2, h3, [title]",
        linkSelector: "a",
        linkAttribute: "href",
        posterSelector: "img, .poster img, picture img",
        posterAttribute: "src"
      })
    );
    setMessage("Common selector candidates applied. Test the source to preview matches.");
  };

  const autoFixDraft = async () => {
    setTestingId(draft.id);
    setMessage(null);
    try {
      const sourceForTest = normalizeForSave(draft, headersText);
      validateDraft(sourceForTest, Boolean(headersText.trim()));
      const result = await onTest(sourceForTest, testQuery || "john wick");
      setTestResult(result);
      const resultSelector = bestSelector(result.detectedSelectors, "result")?.selector;
      const titleSelector = bestSelector(result.detectedSelectors, "title")?.selector;
      const posterSelector = bestSelector(result.detectedSelectors, "poster")?.selector;
      setUndoDraft(draft);
      setUndoHeadersText(headersText);
      setShowAdvanced(true);
      setDraft((current) =>
        normalizeSource({
          ...current,
          sourceKind: "direct",
          requiresJavaScript:
            result.debugInfo?.javascriptProbablyRequired ?? result.fallbackUsed ?? current.requiresJavaScript,
          resultSelector: resultSelector || current.resultSelector || ".movie-card, .card, .item, article, a[href]",
          waitForSelector: resultSelector || current.waitForSelector || current.resultSelector,
          titleSelector: titleSelector || current.titleSelector || ".title, .name, .movie-title, h2, h3, [title]",
          linkSelector: current.linkSelector || "a[href]",
          linkAttribute: current.linkAttribute || "href",
          posterSelector: posterSelector || current.posterSelector,
          posterAttribute: current.posterAttribute || "src",
          loadDelayMs: result.fallbackUsed ? Math.max(current.loadDelayMs ?? 1500, 2500) : current.loadDelayMs,
          parserMode: result.fallbackUsed ? "hybrid" : current.parserMode || "hybrid"
        })
      );
      setMessage(
        resultSelector
          ? "Auto-fix found selector suggestions. Review the preview, then save the source."
          : "Auto-fix could not find strong selectors. Review diagnostics before saving."
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setTestingId(null);
    }
  };

  const runDiagnostics = async () => {
    setDiagnosticsRunning(true);
    setMessage(null);
    try {
      const targets = sources.filter((source) => !source.isDeleted && !source.hidden);
      const results = await Promise.all(
        targets.map(async (source) => ({
          source: normalizeSource(source),
          result: await onTest(normalizeSource(source), testQuery || "john wick")
        }))
      );
      setDiagnosticResults(results);
      setMessage(`Diagnostics complete for ${results.length} source${results.length === 1 ? "" : "s"}.`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setDiagnosticsRunning(false);
    }
  };

  const useCandidate = (candidate: SelectorCandidate) => {
    if (candidate.selectorType === "result") {
      updateDraft("resultSelector", candidate.selector);
      updateDraft("waitForSelector", candidate.selector);
    }
    if (candidate.selectorType === "title") {
      updateDraft("titleSelector", candidate.selector);
    }
    if (candidate.selectorType === "poster") {
      updateDraft("posterSelector", candidate.selector);
    }
  };

  const canUndo =
    JSON.stringify(draft) !== JSON.stringify(undoDraft) ||
    headersText !== undoHeadersText;

  return (
    <section className="view sources-view">
      <header className="view-header">
        <div>
          <h1>Sources</h1>
          <p>
            {activeCount} active, {disabledCount} disabled, {trashCount} in trash
          </p>
        </div>
        <div className="toolbar">
          <button type="button" className="secondary-button" onClick={beginNew}>
            <Plus size={17} />
            <span>Add</span>
          </button>
          <button
            type="button"
            className="secondary-button"
            onClick={() => void handleDefaultAction(onRestoreDefaults, "Default sources restored.")}
          >
            <RefreshCw size={17} />
            <span>Restore defaults</span>
          </button>
          <button
            type="button"
            className="secondary-button"
            onClick={() => {
              if (window.confirm("Reset all default sources to the built-in configs?")) {
                void handleDefaultAction(onResetAllDefaults, "Default sources reset.");
              }
            }}
          >
            <RotateCcw size={17} />
            <span>Reset defaults</span>
          </button>
          <button
            type="button"
            className="secondary-button"
            onClick={() => void runDiagnostics()}
            disabled={diagnosticsRunning}
          >
            {diagnosticsRunning ? <RefreshCw className="spin" size={17} /> : <TestTube2 size={17} />}
            <span>Diagnostics</span>
          </button>
          {trashCount > 0 && (
            <button
              type="button"
              className="secondary-button danger"
              onClick={() => {
                if (window.confirm("Permanently delete every source in Trash?")) {
                  void handleDefaultAction(onEmptyTrash, "Trash emptied.");
                }
              }}
            >
              <Trash2 size={17} />
              <span>Empty trash</span>
            </button>
          )}
          <button
            type="button"
            className="secondary-button"
            onClick={() => downloadJson(sourceJsonExportName(), sources)}
          >
            <Download size={17} />
            <span>Export</span>
          </button>
          <label className="secondary-button file-button">
            <Upload size={17} />
            <span>Import</span>
            <input
              type="file"
              accept="application/json,.json"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) {
                  void importFile(file);
                }
                event.currentTarget.value = "";
              }}
            />
          </label>
        </div>
      </header>

      <section className="quick-add-panel">
        <div>
          <h2>Quick Add Website</h2>
          <p>Paste a site or search URL. CineFinder creates a simple WebView source.</p>
        </div>
        <div className="quick-add-row">
          <input
            value={quickUrl}
            onChange={(event) => setQuickUrl(event.target.value)}
            placeholder="https://example.com/search?q=movie"
            aria-label="Website URL"
          />
          <button className="secondary-button" type="button" onClick={autoFillFromUrl}>
            <Sparkles size={17} />
            <span>Auto fill</span>
          </button>
          <button
            className="primary-button"
            type="button"
            disabled={busy || !quickUrl.trim()}
            onClick={() => void quickAddFromUrl()}
          >
            <Plus size={17} />
            <span>Add</span>
          </button>
        </div>
      </section>

      <div className="source-tab-row" role="tablist" aria-label="Source filters">
        <button
          className={cx("chip", sourceTab === "active" && "is-selected")}
          type="button"
          onClick={() => setSourceTab("active")}
        >
          Active {activeCount}
        </button>
        <button
          className={cx("chip", sourceTab === "disabled" && "is-selected")}
          type="button"
          onClick={() => setSourceTab("disabled")}
        >
          Disabled {disabledCount}
        </button>
        <button
          className={cx("chip", sourceTab === "trash" && "is-selected")}
          type="button"
          onClick={() => setSourceTab("trash")}
        >
          Trash {trashCount}
        </button>
      </div>

      {diagnosticResults.length > 0 && (
        <section className="diagnostics-panel">
          <div>
            <strong>Source Diagnostics</strong>
            <span>Query: {testQuery || "john wick"}</span>
          </div>
          <div className="diagnostics-grid">
            {diagnosticResults.map(({ source, result }) => (
              <article key={source.id} className="diagnostics-card">
                <strong>{source.name}</strong>
                <span>{result.finalSearchUrl || "n/a"}</span>
                <small>Status: {result.rawStatus || (result.ok ? "loaded" : "failed")}</small>
                <small>Parser: {result.debugInfo?.parserModeUsed || source.parserMode || "hybrid"}</small>
                <small>Candidates: {result.resultCount}</small>
                <small>Best score: {result.debugInfo?.bestScore ?? result.bestMatch?.score ?? "n/a"}</small>
                <small>Final action: {result.debugInfo?.finalAction || (result.fallbackUsed ? "search fallback" : "exact page")}</small>
                <small>Top titles: {result.debugInfo?.candidateTitles?.slice(0, 5).join(", ") || result.previewResults?.slice(0, 5).map((item) => item.title).join(", ") || "none"}</small>
                <small>Top URLs: {result.debugInfo?.candidateLinks?.slice(0, 5).join(", ") || result.previewResults?.slice(0, 5).map((item) => item.url).join(", ") || "none"}</small>
              </article>
            ))}
          </div>
        </section>
      )}

      <div className="source-layout">
        <div className="source-list" aria-label="Sources">
          {sortedSources.map((source) => (
            <article className={cx("source-list-item", source.hidden && "is-muted")} key={source.id}>
              <div>
                <strong>{source.name || "Untitled source"}</strong>
                <span>{source.baseUrl}</span>
                <div className="source-badge-row">
                  <small className="source-kind-badge">
                    {source.isDefault ? "Default" : "Custom"}
                  </small>
                  {source.userModified && <small className="source-kind-badge warm">Modified</small>}
                  {!source.enabled && <small className="source-kind-badge muted">Disabled</small>}
                  {source.isDeleted && <small className="source-kind-badge danger">Trash</small>}
                  {source.hidden && <small className="source-kind-badge danger">Hidden</small>}
                </div>
              </div>
              <div className="source-list-actions">
                {source.isDeleted ? (
                  <>
                    <button
                      className="secondary-button compact"
                      type="button"
                      onClick={() =>
                        void handleDefaultAction(
                          () => onRestore(source.id),
                          `${source.name} restored.`
                        )
                      }
                      title="Restore source"
                    >
                      <Undo2 size={15} />
                      <span>Restore</span>
                    </button>
                    <button
                      className="icon-button danger"
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Permanently delete ${source.name}?`)) {
                          void handleDefaultAction(
                            () => onPermanentDelete(source.id),
                            `${source.name} permanently deleted.`
                          );
                        }
                      }}
                      title="Permanently delete source"
                    >
                      <Trash2 size={16} />
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      className="toggle-button"
                      type="button"
                      onClick={() =>
                        void onSave({
                          ...source,
                          enabled: !source.enabled,
                          hidden: false
                        })
                      }
                      title={source.enabled && !source.hidden ? "Disable source" : "Enable source"}
                    >
                      {source.enabled && !source.hidden ? <Check size={16} /> : <X size={16} />}
                      <span>{source.enabled && !source.hidden ? "On" : "Off"}</span>
                    </button>
                    <button
                      className="icon-button"
                      type="button"
                      onClick={() => void testDraft(source)}
                      title="Test source"
                      disabled={testingId === source.id}
                    >
                      {testingId === source.id ? (
                        <RefreshCw className="spin" size={16} />
                      ) : (
                        <TestTube2 size={16} />
                      )}
                    </button>
                    <button
                      className="icon-button"
                      type="button"
                      onClick={() => beginEdit(source)}
                      title="Edit source"
                    >
                      <Pencil size={16} />
                    </button>
                    <button
                      className="icon-button"
                      type="button"
                      onClick={() => void onDuplicate(source)}
                      title="Duplicate source"
                    >
                      <Copy size={16} />
                    </button>
                    {source.isDefault && (
                      <button
                        className="icon-button"
                        type="button"
                        onClick={() => {
                          if (window.confirm(`Reset ${source.name} to the built-in default config?`)) {
                            void handleDefaultAction(
                              () => onResetSource(source.id),
                              `${source.name} reset.`
                            );
                          }
                        }}
                        title="Reset this default source"
                      >
                        <RotateCcw size={16} />
                      </button>
                    )}
                    <button
                      className="icon-button danger"
                      type="button"
                      onClick={() => {
                        if (window.confirm(`Move ${source.name} to Trash?`)) {
                          void onDelete(source.id);
                        }
                      }}
                      title="Move source to Trash"
                    >
                      <Trash2 size={16} />
                    </button>
                  </>
                )}
              </div>
            </article>
          ))}
          {sortedSources.length === 0 && (
            <div className="empty-panel compact-empty">
              <Plus size={18} />
              <strong>{sourceTab === "trash" ? "Trash is empty" : "No sources"}</strong>
              <span>
                {sourceTab === "active"
                  ? "Enable a source or add a new config."
                  : sourceTab === "disabled"
                    ? "Disabled sources will appear here."
                    : "Deleted sources will appear here."}
              </span>
            </div>
          )}
        </div>

        <form
          className="source-editor"
          onSubmit={(event) => {
            event.preventDefault();
            void saveDraft();
          }}
        >
          <div className="editor-title-row">
            <div>
              <h2>{editingId ? "Edit Source" : "Add Source"}</h2>
              <p>{draft.id}</p>
            </div>
            <label className="switch">
              <input
                type="checkbox"
                checked={draft.enabled && !draft.hidden}
                onChange={(event) => {
                  updateDraft("enabled", event.target.checked);
                  if (event.target.checked) {
                    updateDraft("hidden", false);
                  }
                }}
              />
              <span />
            </label>
          </div>

          <div className="source-mode-grid" role="radiogroup" aria-label="Editor mode">
            <button
              type="button"
              className={cx("source-mode-option", !showAdvanced && "is-selected")}
              onClick={() => switchEditorMode(false)}
            >
              <Globe2 size={18} />
              <strong>Simple Mode</strong>
              <span>Name, URL, open behavior</span>
            </button>
            <button
              type="button"
              className={cx("source-mode-option", showAdvanced && "is-selected")}
              onClick={() => switchEditorMode(true)}
            >
              <Film size={18} />
              <strong>Advanced Mode</strong>
              <span>CSS selectors and headers</span>
            </button>
          </div>

          <div className="template-row" aria-label="Source templates">
            {sourceTemplates.map((template) => (
              <button
                type="button"
                className="chip"
                key={template.label}
                onClick={() => applyTemplate(template)}
              >
                {template.label}
              </button>
            ))}
          </div>

          {draft.isDefault && (
            <div className="source-mode-note">
              This is a seeded default source. You can edit it like any custom source; your local
              changes are saved until you reset it.
            </div>
          )}

          <div className="form-grid two">
            <label>
              <span>Source name</span>
              <input
                value={draft.name}
                onChange={(event) => updateDraft("name", event.target.value)}
                required
              />
            </label>
            <label>
              <span>Source type</span>
              <select
                value={sourceType}
                onChange={(event) =>
                  updateDraft("sourceType", event.target.value as SourceType)
                }
              >
                <option value="search">Search page only</option>
                <option value="directPage">Direct video page</option>
                <option value="webviewOnly">WebView only</option>
              </select>
            </label>
            <label>
              <span>Parser mode</span>
              <select
                value={draft.parserMode || "hybrid"}
                onChange={(event) => updateDraft("parserMode", event.target.value as ParserMode)}
              >
                <option value="hybrid">Hybrid</option>
                <option value="static">Static HTML parser</option>
                <option value="webview">JavaScript/WebView parser</option>
                <option value="fallbackOnly">WebView fallback only</option>
              </select>
            </label>
          </div>

          <label>
            <span>Base URL</span>
            <input
              value={draft.baseUrl}
              onChange={(event) => updateDraft("baseUrl", event.target.value)}
              placeholder="https://example.com"
              required
            />
          </label>

          <label>
            <span>{sourceType === "directPage" ? "Page URL / Search URL" : "Search URL pattern"}</span>
            <input
              value={draft.searchUrl}
              onChange={(event) => updateDraft("searchUrl", event.target.value)}
              placeholder={
                sourceType === "directPage"
                  ? "https://example.com/movie-page"
                  : "https://example.com/search/{query}"
              }
              required={sourceType !== "directPage"}
            />
            <em className="field-help">
              Use {"{query}"} where the movie name should go. Example:
              https://example.com/search/{"{query}"}
            </em>
          </label>

          <div className="form-grid two">
            <label>
              <span>Open behavior</span>
              <select
                value={draft.sourceOpenBehavior || "webview"}
                onChange={(event) =>
                  updateDraft(
                    "sourceOpenBehavior",
                    event.target.value as SourceOpenBehavior
                  )
                }
              >
                <option value="webview">In-app WebView</option>
                <option value="nativeThenWebview">Native player if possible, otherwise WebView</option>
              </select>
            </label>
            <label>
              <span>Result target</span>
              <select
                value={draft.resultOpenBehavior || "result_page"}
                onChange={(event) =>
                  updateDraft("resultOpenBehavior", event.target.value as ResultPageBehavior)
                }
              >
                <option value="result_page">Best parsed result page</option>
                <option value="search_page">Search page fallback</option>
              </select>
            </label>
          </div>

          <div className="form-grid two">
            <label>
              <span>Ambiguous query behavior</span>
              <select
                value={draft.ambiguousQueryBehavior || "show_choices"}
                onChange={(event) =>
                  updateDraft(
                    "ambiguousQueryBehavior",
                    event.target.value as AmbiguousQueryBehavior
                  )
                }
              >
                <option value="show_choices">Show choices</option>
                <option value="open_search_page">Open search page</option>
              </select>
            </label>
            <label>
              <span>Exact match threshold</span>
              <input
                type="number"
                min={50}
                max={100}
                step={1}
                value={draft.exactMatchThreshold ?? 85}
                onChange={(event) => updateDraft("exactMatchThreshold", Number(event.target.value))}
              />
            </label>
          </div>

          <div className="form-grid two">
            <label className="checkbox-row inline-control">
              <input
                type="checkbox"
                checked={draft.requiresJavaScript}
                onChange={(event) => updateDraft("requiresJavaScript", event.target.checked)}
              />
              <span>Requires JavaScript</span>
            </label>
            <label className="checkbox-row inline-control">
              <input
                type="checkbox"
                checked={draft.autoOpenBestMatch !== false}
                onChange={(event) => updateDraft("autoOpenBestMatch", event.target.checked)}
              />
              <span>Auto-open best match when specific</span>
            </label>
          </div>

          {!showAdvanced && (
            <div className="source-mode-note">
              Selectors are optional. If CineFinder cannot parse movie cards, it will show one clean
              card that opens the generated source search page inside the app.
            </div>
          )}

          <details>
            <summary>Advanced reliability</summary>
            <div className="form-grid two">
              <label>
                <span>Load delay ms</span>
                <input
                  type="number"
                  min={0}
                  max={10000}
                  step={250}
                  value={draft.loadDelayMs ?? 1500}
                  onChange={(event) => updateDraft("loadDelayMs", Number(event.target.value))}
                />
              </label>
              <label>
                <span>Max retries</span>
                <input
                  type="number"
                  min={0}
                  max={5}
                  step={1}
                  value={draft.maxRetries ?? 2}
                  onChange={(event) => updateDraft("maxRetries", Number(event.target.value))}
                />
              </label>
              <label>
                <span>Request timeout ms</span>
                <input
                  type="number"
                  min={3000}
                  max={60000}
                  step={1000}
                  value={draft.requestTimeoutMs ?? 15000}
                  onChange={(event) => updateDraft("requestTimeoutMs", Number(event.target.value))}
                />
              </label>
              <label>
                <span>Wait selector</span>
                <input
                  value={draft.waitForSelector ?? ""}
                  onChange={(event) => updateDraft("waitForSelector", event.target.value)}
                  placeholder=".movie-card"
                />
              </label>
            </div>
          </details>

          {showAdvanced && (
            <>
              <div className="source-mode-grid" role="radiogroup" aria-label="Parsing mode">
                <button
                  type="button"
                  className={cx("source-mode-option", draftKind === "web" && "is-selected")}
                  onClick={() => switchSourceKind("web")}
                >
                  <Globe2 size={18} />
                  <strong>WebView source</strong>
                  <span>One source search card</span>
                </button>
                <button
                  type="button"
                  className={cx("source-mode-option", draftKind === "direct" && "is-selected")}
                  onClick={() => switchSourceKind("direct")}
                >
                  <Film size={18} />
                  <strong>Selector parser</strong>
                  <span>Parse cards with CSS</span>
                </button>
              </div>

              <div className="editor-actions wrap">
                <button
                  className="secondary-button"
                  type="button"
                  onClick={applyCommonSelectors}
                >
                  <Wand2 size={17} />
                  <span>Auto-detect selectors</span>
                </button>
                {resultSelectorCandidates.slice(0, 5).map((selector) => (
                  <button
                    className="chip"
                    type="button"
                    key={selector}
                    onClick={() => {
                      updateDraft("resultSelector", selector);
                      updateDraft("waitForSelector", selector);
                    }}
                  >
                    {selector}
                  </button>
                ))}
              </div>

              <details open>
                <summary>Result selectors</summary>
                <div className="form-grid two">
                  <label>
                    <span>Result selector</span>
                    <input
                      value={draft.resultSelector}
                      onChange={(event) => updateDraft("resultSelector", event.target.value)}
                      placeholder=".movie-card, .card, .item, article"
                    />
                  </label>
                  <label>
                    <span>Title selector</span>
                    <input
                      value={draft.titleSelector ?? ""}
                      onChange={(event) => updateDraft("titleSelector", event.target.value)}
                      placeholder=".title, h2, h3, [title]"
                    />
                  </label>
                  <label>
                    <span>Link selector</span>
                    <input
                      value={draft.linkSelector ?? ""}
                      onChange={(event) => updateDraft("linkSelector", event.target.value)}
                      placeholder="a"
                    />
                  </label>
                  <label>
                    <span>Link attribute</span>
                    <input
                      value={draft.linkAttribute ?? ""}
                      onChange={(event) => updateDraft("linkAttribute", event.target.value)}
                      placeholder="href"
                    />
                  </label>
                  <label>
                    <span>Poster selector</span>
                    <input
                      value={draft.posterSelector ?? ""}
                      onChange={(event) => updateDraft("posterSelector", event.target.value)}
                      placeholder="img, .poster img"
                    />
                  </label>
                  <label>
                    <span>Poster attribute</span>
                    <input
                      value={draft.posterAttribute ?? ""}
                      onChange={(event) => updateDraft("posterAttribute", event.target.value)}
                      placeholder="src"
                    />
                  </label>
                  <label>
                    <span>Year selector</span>
                    <input
                      value={draft.yearSelector ?? ""}
                      onChange={(event) => updateDraft("yearSelector", event.target.value)}
                      placeholder=".year"
                    />
                  </label>
                  <label>
                    <span>Description selector</span>
                    <input
                      value={draft.descriptionSelector ?? ""}
                      onChange={(event) => updateDraft("descriptionSelector", event.target.value)}
                      placeholder=".description"
                    />
                  </label>
                </div>
              </details>

              <details>
                <summary>Exact watching page</summary>
                <div className="form-grid two">
                  <label className="checkbox-row inline-control">
                    <input
                      type="checkbox"
                      checked={draft.autoResolveWatchPage !== false}
                      onChange={(event) => {
                        updateDraft("autoResolveWatchPage", event.target.checked);
                        updateDraft("autoOpenWatchButton", event.target.checked);
                      }}
                    />
                    <span>Auto-resolve watch page</span>
                  </label>
                  <label>
                    <span>Watch button selector</span>
                    <input
                      value={draft.watchButtonSelector ?? ""}
                      onChange={(event) => updateDraft("watchButtonSelector", event.target.value)}
                      placeholder="a.watch, a[href*='watch'], .play a"
                    />
                  </label>
                  <label>
                    <span>Player selector</span>
                    <input
                      value={draft.playerSelector ?? ""}
                      onChange={(event) => updateDraft("playerSelector", event.target.value)}
                      placeholder="video, iframe"
                    />
                  </label>
                  <label>
                    <span>Watch text patterns</span>
                    <textarea
                      value={(draft.watchLinkTextPatterns || []).join("\n")}
                      onChange={(event) =>
                        updateDraft(
                          "watchLinkTextPatterns",
                          event.target.value
                            .split(/\r?\n|,/)
                            .map((item) => item.trim())
                            .filter(Boolean)
                        )
                      }
                      placeholder={"watch full movie\nwatch online\nwatch now\nplay\nstart watching\nсмотреть\nсмотреть онлайн"}
                      rows={5}
                    />
                  </label>
                  <label>
                    <span>Max resolve steps</span>
                    <input
                      type="number"
                      min={0}
                      max={5}
                      step={1}
                      value={draft.maxResolveSteps ?? draft.maxWatchResolveSteps ?? 2}
                      onChange={(event) => {
                        updateDraft("maxResolveSteps", Number(event.target.value));
                        updateDraft("maxWatchResolveSteps", Number(event.target.value));
                      }}
                    />
                  </label>
                  <label>
                    <span>Resolve delay ms</span>
                    <input
                      type="number"
                      min={0}
                      max={10000}
                      step={250}
                      value={draft.resolveDelayMs ?? 1500}
                      onChange={(event) =>
                        updateDraft("resolveDelayMs", Number(event.target.value))
                      }
                    />
                  </label>
                  <label>
                    <span>Season selector</span>
                    <input
                      value={draft.seasonSelector ?? ""}
                      onChange={(event) => updateDraft("seasonSelector", event.target.value)}
                      placeholder=".season, [data-season]"
                    />
                  </label>
                  <label>
                    <span>Episode selector</span>
                    <input
                      value={draft.episodeSelector ?? ""}
                      onChange={(event) => updateDraft("episodeSelector", event.target.value)}
                      placeholder=".episode, [data-episode]"
                    />
                  </label>
                  <label className="checkbox-row inline-control">
                    <input
                      type="checkbox"
                      checked={draft.autoOpenWatchButton !== false}
                      onChange={(event) =>
                        updateDraft("autoOpenWatchButton", event.target.checked)
                      }
                    />
                    <span>Match watch/play text patterns</span>
                  </label>
                </div>
              </details>

              <details>
                <summary>Media selectors</summary>
                <div className="form-grid two">
                  <label>
                    <span>Video selector</span>
                    <input
                      value={draft.videoSelector ?? ""}
                      onChange={(event) => updateDraft("videoSelector", event.target.value)}
                      placeholder="video source, video[src], source[src]"
                    />
                  </label>
                  <label>
                    <span>Video attribute</span>
                    <input
                      value={draft.videoAttribute ?? ""}
                      onChange={(event) => updateDraft("videoAttribute", event.target.value)}
                      placeholder="src"
                    />
                  </label>
                  <label>
                    <span>Iframe selector</span>
                    <input
                      value={draft.iframeSelector ?? ""}
                      onChange={(event) => updateDraft("iframeSelector", event.target.value)}
                      placeholder="iframe"
                    />
                  </label>
                  <label>
                    <span>Iframe attribute</span>
                    <input
                      value={draft.iframeAttribute ?? ""}
                      onChange={(event) => updateDraft("iframeAttribute", event.target.value)}
                      placeholder="src"
                    />
                  </label>
                  <label>
                    <span>Subtitle selector</span>
                    <input
                      value={draft.subtitleSelector ?? ""}
                      onChange={(event) => updateDraft("subtitleSelector", event.target.value)}
                      placeholder="track[kind='subtitles']"
                    />
                  </label>
                  <label>
                    <span>Subtitle attribute</span>
                    <input
                      value={draft.subtitleAttribute ?? ""}
                      onChange={(event) => updateDraft("subtitleAttribute", event.target.value)}
                      placeholder="src"
                    />
                  </label>
                  <label>
                    <span>Subtitle language</span>
                    <input
                      value={draft.subtitleLanguageAttribute ?? ""}
                      onChange={(event) =>
                        updateDraft("subtitleLanguageAttribute", event.target.value)
                      }
                      placeholder="srclang"
                    />
                  </label>
                  <label>
                    <span>Audio language selector</span>
                    <input
                      value={draft.audioLanguageSelector ?? ""}
                      onChange={(event) => updateDraft("audioLanguageSelector", event.target.value)}
                      placeholder="track[kind='captions']"
                    />
                  </label>
                  <label>
                    <span>Download selector</span>
                    <input
                      value={draft.downloadSelector ?? ""}
                      onChange={(event) => updateDraft("downloadSelector", event.target.value)}
                      placeholder="a.download"
                    />
                  </label>
                  <label>
                    <span>Download attribute</span>
                    <input
                      value={draft.downloadAttribute ?? ""}
                      onChange={(event) => updateDraft("downloadAttribute", event.target.value)}
                      placeholder="href"
                    />
                  </label>
                </div>
              </details>

              <details>
                <summary>Headers JSON</summary>
                <label>
                  <span>Headers</span>
                  <textarea
                    value={headersText}
                    onChange={(event) => setHeadersText(event.target.value)}
                    placeholder={'{\n  "Accept-Language": "en-US,en;q=0.9"\n}'}
                    rows={5}
                  />
                </label>
              </details>
            </>
          )}

          <label>
            <span>Resolve test query</span>
            <input
              value={testQuery}
              onChange={(event) => setTestQuery(event.target.value)}
              placeholder="gravity falls"
            />
          </label>

          <div className="editor-actions">
            <button className="primary-button" type="submit" disabled={busy}>
              <Save size={17} />
              <span>{busy ? "Saving" : "Save"}</span>
            </button>
            <button
              className="secondary-button"
              type="button"
              onClick={() => void testDraft()}
              disabled={testingId === draft.id}
            >
              {testingId === draft.id ? (
                <RefreshCw className="spin" size={17} />
              ) : (
                <TestTube2 size={17} />
              )}
              <span>Test</span>
            </button>
            <button
              className="secondary-button"
              type="button"
              onClick={() => void autoFixDraft()}
              disabled={testingId === draft.id}
            >
              {testingId === draft.id ? (
                <RefreshCw className="spin" size={17} />
              ) : (
                <Wand2 size={17} />
              )}
              <span>Auto-fix source</span>
            </button>
            {draft.isDefault && (
              <button
                className="secondary-button"
                type="button"
                onClick={() => {
                  if (window.confirm("Reset this source to the built-in default config?")) {
                    void handleDefaultAction(
                      () => onResetSource(draft.id),
                      `${draft.name} reset.`
                    );
                  }
                }}
              >
                <RotateCcw size={17} />
                <span>Reset</span>
              </button>
            )}
            <button className="ghost-button" type="button" onClick={beginNew}>
              <X size={17} />
              <span>Clear</span>
            </button>
            <button
              className="ghost-button"
              type="button"
              onClick={undoDraftChanges}
              disabled={!canUndo}
            >
              <Undo2 size={17} />
              <span>Undo</span>
            </button>
          </div>

          {message && <div className="form-message">{message}</div>}
          {testResult && (
            <div className="test-preview">
              <strong>Test Source Preview</strong>
              <label>
                <span>Sample query</span>
                <input
                  value={testQuery}
                  onChange={(event) => setTestQuery(event.target.value)}
                  placeholder="gravity falls"
                />
              </label>
              <span>URL: {testResult.finalSearchUrl || "n/a"}</span>
              <span>Status: {testResult.rawStatus || (testResult.ok ? "loaded" : "failed")}</span>
              <span>Parser mode: {testResult.debugInfo?.parserModeUsed || draft.parserMode || "hybrid"}</span>
              <span>HTML length: {testResult.debugInfo?.htmlLength ?? "n/a"}</span>
              <span>Selector matches: {testResult.selectorMatchCount ?? testResult.resultCount}</span>
              <span>Candidate titles: {testResult.debugInfo?.candidateTitles?.slice(0, 5).join(", ") || "none"}</span>
              <span>Best score: {testResult.debugInfo?.bestScore ?? "n/a"}</span>
              <span>Final action: {testResult.debugInfo?.finalAction || "n/a"}</span>
              <span>Fallback provider card: {testResult.fallbackUsed ? "yes" : "no"}</span>
              {testResult.querySpecificity && (
                <span>
                  Query decision: {testResult.ambiguous ? "ambiguous" : "specific"} -{" "}
                  {testResult.querySpecificity}
                </span>
              )}
              {testResult.bestMatch && (
                <span>Best match: {testResult.bestMatch.title}</span>
              )}
              {testResult.finalOpenUrl && <span>Final open URL: {testResult.finalOpenUrl}</span>}
              {(testResult.previewResults || []).slice(0, 5).map((item, index) => (
                <div className="test-preview-row" key={`${item.url}-${index}`}>
                  <b>
                    {item.title || "Untitled"}
                    {typeof item.score === "number" ? ` (${Math.round(item.score)}%)` : ""}
                    {item.year ? ` ${item.year}` : ""}
                  </b>
                  {item.confidenceReason && <small>{item.confidenceReason}</small>}
                  <small>{item.url}</small>
                </div>
              ))}
              {Boolean(testResult.detectedSelectors?.length) && (
                <div className="selector-candidates">
                  <strong>Detected selector candidates</strong>
                  {testResult.detectedSelectors?.map((candidate) => (
                    <button
                      className="ghost-button"
                      type="button"
                      key={`${candidate.selectorType}-${candidate.selector}`}
                      onClick={() => useCandidate(candidate)}
                    >
                      <span>
                        {candidate.selectorType}: {candidate.selector} ({candidate.matchCount})
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </form>
      </div>
    </section>
  );

  async function handleDefaultAction(action: () => Promise<void>, success: string) {
    setBusy(true);
    setMessage(null);
    try {
      await action();
      beginNew();
      setMessage(success);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }
}

interface SourceTemplate {
  label: string;
  advanced: boolean;
  patch: Partial<SourceConfig>;
}

function bestSelector(
  candidates: SelectorCandidate[] | undefined,
  selectorType: SelectorCandidate["selectorType"]
): SelectorCandidate | null {
  return (
    candidates
      ?.filter((candidate) => candidate.selectorType === selectorType)
      .sort((left, right) => right.matchCount - left.matchCount)[0] ?? null
  );
}

const sourceTemplates: SourceTemplate[] = [
  {
    label: "Basic search page",
    advanced: false,
    patch: {
      sourceKind: "web",
      sourceType: "search",
      sourceOpenBehavior: "webview",
      parserMode: "hybrid",
      resultOpenBehavior: "result_page",
      ambiguousQueryBehavior: "show_choices",
      requiresJavaScript: true,
      autoOpenBestMatch: true,
      autoResolveWatchPage: true,
      autoOpenWatchButton: true,
      maxWatchResolveSteps: 2,
      maxResolveSteps: 2,
      resolveDelayMs: 1500,
      exactMatchThreshold: 85,
      resultSelector: "",
      waitForSelector: ""
    }
  },
  {
    label: "Search page with cards",
    advanced: true,
    patch: {
      sourceKind: "direct",
      sourceType: "search",
      sourceOpenBehavior: "nativeThenWebview",
      parserMode: "hybrid",
      resultOpenBehavior: "result_page",
      ambiguousQueryBehavior: "show_choices",
      requiresJavaScript: false,
      autoOpenBestMatch: true,
      autoResolveWatchPage: true,
      autoOpenWatchButton: true,
      maxWatchResolveSteps: 2,
      maxResolveSteps: 2,
      resolveDelayMs: 1500,
      exactMatchThreshold: 85,
      resultSelector: ".movie-card, .card, .item, article",
      waitForSelector: ".movie-card, .card, .item, article",
      titleSelector: ".title, h2, h3, [title]",
      linkSelector: "a",
      linkAttribute: "href",
      posterSelector: "img",
      posterAttribute: "src"
    }
  },
  {
    label: "Direct video URL page",
    advanced: false,
    patch: {
      sourceKind: "web",
      sourceType: "directPage",
      sourceOpenBehavior: "nativeThenWebview",
      parserMode: "hybrid",
      resultOpenBehavior: "result_page",
      ambiguousQueryBehavior: "show_choices",
      requiresJavaScript: true,
      autoOpenBestMatch: true,
      autoResolveWatchPage: true,
      autoOpenWatchButton: true,
      maxResolveSteps: 2,
      resolveDelayMs: 1500,
      searchUrl: ""
    }
  },
  {
    label: "WebView only",
    advanced: false,
    patch: {
      sourceKind: "web",
      sourceType: "webviewOnly",
      sourceOpenBehavior: "webview",
      parserMode: "fallbackOnly",
      resultOpenBehavior: "search_page",
      ambiguousQueryBehavior: "open_search_page",
      requiresJavaScript: true,
      autoResolveWatchPage: true,
      resultSelector: "",
      titleSelector: "",
      linkSelector: "",
      videoSelector: "",
      loadDelayMs: 2000,
      maxRetries: 2,
      requestTimeoutMs: 15000
    }
  },
  {
    label: "JavaScript-heavy site",
    advanced: false,
    patch: {
      sourceKind: "web",
      sourceType: "search",
      sourceOpenBehavior: "webview",
      parserMode: "hybrid",
      resultOpenBehavior: "result_page",
      ambiguousQueryBehavior: "show_choices",
      requiresJavaScript: true,
      autoOpenBestMatch: true,
      autoResolveWatchPage: true,
      autoOpenWatchButton: true,
      maxResolveSteps: 2,
      resolveDelayMs: 1500,
      loadDelayMs: 2500,
      maxRetries: 2,
      requestTimeoutMs: 15000
    }
  }
];

function normalizeForSave(source: SourceConfig, headersText: string): SourceConfig {
  const sourceType = source.sourceType || "search";
  const searchUrl = sourceType === "directPage" && !source.searchUrl.trim()
    ? source.baseUrl
    : source.searchUrl;
  return normalizeSource({
    ...source,
    searchUrl,
    method: "GET",
    headers: parseHeaders(headersText),
    hidden: Boolean(source.hidden) && !source.enabled,
    isDeleted: Boolean(source.isDeleted),
    deletedAt: source.isDeleted ? source.deletedAt || new Date().toISOString() : null
  });
}

function validateDraft(source: SourceConfig, validateHeaders: boolean): void {
  if (!source.name.trim()) {
    throw new Error("Source name is required.");
  }
  const baseUrl = parseHttpUrl(source.baseUrl, "Base URL");
  const sourceType = source.sourceType || "search";
  if (sourceType !== "directPage") {
    if (!source.searchUrl.trim()) {
      throw new Error("Search URL is required.");
    }
    if (!source.searchUrl.includes("{query}") && !source.searchUrl.includes("{slug}")) {
      throw new Error("Search URL must include {query} where the movie name should go.");
    }
  }
  if (source.searchUrl.trim()) {
    parseHttpUrl(
      source.searchUrl.replaceAll("{query}", "test").replaceAll("{slug}", "test"),
      "Search URL",
      baseUrl.href
    );
  }
  if (validateHeaders) {
    parseHeaders(JSON.stringify(source.headers || {}));
  }
}

function parseHttpUrl(value: string, label: string, baseUrl?: string): URL {
  try {
    const url = new URL(value.trim(), baseUrl);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      throw new Error();
    }
    return url;
  } catch {
    throw new Error(`${label} must start with http:// or https://.`);
  }
}
