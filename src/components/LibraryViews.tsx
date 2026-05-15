import { CircleSlash, Eraser, Play, Star, Trash2 } from "lucide-react";
import type { Favorite, HistoryItem, SearchResult } from "../types";
import {
  favoriteToResult,
  formatDate,
  historyToResult
} from "../utils";

interface FavoritesViewProps {
  favorites: Favorite[];
  onOpen: (result: SearchResult) => void;
  onRemove: (favoriteId: string) => void;
}

interface HistoryViewProps {
  history: HistoryItem[];
  onOpen: (result: SearchResult) => void;
  onRemove: (historyId: string) => void;
  onClear: () => void;
}

export function FavoritesView({ favorites, onOpen, onRemove }: FavoritesViewProps) {
  return (
    <section className="view">
      <header className="view-header">
        <div>
          <h1>Favorites</h1>
          <p>{favorites.length} saved item{favorites.length === 1 ? "" : "s"}</p>
        </div>
      </header>

      {favorites.length === 0 ? (
        <div className="empty-panel">
          <Star size={20} />
          <strong>No favorites</strong>
          <span>Saved results will appear here.</span>
        </div>
      ) : (
        <div className="library-grid">
          {favorites.map((favorite) => (
            <LibraryCard
              key={favorite.id}
              title={favorite.title}
              sourceName={favorite.sourceName}
              url={favorite.url}
              posterUrl={favorite.posterUrl}
              date={favorite.createdAt}
              onOpen={() => onOpen(favoriteToResult(favorite))}
              onRemove={() => onRemove(favorite.id)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

export function HistoryView({ history, onOpen, onRemove, onClear }: HistoryViewProps) {
  return (
    <section className="view">
      <header className="view-header">
        <div>
          <h1>History</h1>
          <p>{history.length} recent item{history.length === 1 ? "" : "s"}</p>
        </div>
        {history.length > 0 && (
          <button type="button" className="secondary-button danger-button" onClick={onClear}>
            <Eraser size={17} />
            <span>Clear</span>
          </button>
        )}
      </header>

      {history.length === 0 ? (
        <div className="empty-panel">
          <CircleSlash size={20} />
          <strong>No history</strong>
          <span>Opened results will appear here.</span>
        </div>
      ) : (
        <div className="library-grid">
          {history.map((item) => (
            <LibraryCard
              key={item.id}
              title={item.title}
              sourceName={item.sourceName}
              url={item.url}
              posterUrl={item.posterUrl}
              date={item.lastOpenedAt}
              progress={
                item.durationSeconds > 0
                  ? item.playbackPositionSeconds / item.durationSeconds
                  : undefined
              }
              onOpen={() => onOpen(historyToResult(item))}
              onRemove={() => onRemove(item.id)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function LibraryCard({
  title,
  sourceName,
  url,
  posterUrl,
  date,
  progress,
  onOpen,
  onRemove
}: {
  title: string;
  sourceName: string;
  url: string;
  posterUrl?: string | null;
  date?: string | null;
  progress?: number;
  onOpen: () => void;
  onRemove: () => void;
}) {
  return (
    <article className="library-card">
      <div className="poster-slot small">
        {posterUrl ? <img alt="" src={posterUrl} loading="lazy" /> : <span>{title[0]}</span>}
      </div>
      <div>
        <div className="library-title-row">
          <h3>{title}</h3>
          <span className="source-badge">{sourceName}</span>
        </div>
        <p>{url}</p>
        {date && <small>{formatDate(date)}</small>}
        {typeof progress === "number" && progress > 0 && (
          <div className="progress-line">
            <span style={{ width: `${Math.max(3, Math.min(100, progress * 100))}%` }} />
          </div>
        )}
      </div>
      <div className="library-actions">
        <button className="primary-button compact" type="button" onClick={onOpen}>
          <Play size={16} />
          <span>Open</span>
        </button>
        <button className="icon-button danger" type="button" onClick={onRemove} title="Remove">
          <Trash2 size={16} />
        </button>
      </div>
    </article>
  );
}
