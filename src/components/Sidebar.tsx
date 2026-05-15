import {
  Download,
  Film,
  History,
  Search,
  Settings,
  SlidersHorizontal,
  Star
} from "lucide-react";
import type { ViewKey } from "../types";
import { cx } from "../utils";

interface SidebarProps {
  activeView: ViewKey;
  onViewChange: (view: ViewKey) => void;
  sourcesCount: number;
  favoritesCount: number;
  historyCount: number;
}

const navItems: Array<{
  key: ViewKey;
  label: string;
  icon: typeof Search;
}> = [
  { key: "search", label: "Search", icon: Search },
  { key: "sources", label: "Sources", icon: SlidersHorizontal },
  { key: "favorites", label: "Favorites", icon: Star },
  { key: "history", label: "History", icon: History },
  { key: "downloads", label: "Downloads", icon: Download },
  { key: "settings", label: "Settings", icon: Settings }
];

export function Sidebar({
  activeView,
  onViewChange,
  sourcesCount,
  favoritesCount,
  historyCount
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <Film size={22} strokeWidth={2.3} />
        </div>
        <div>
          <strong>CineFinder</strong>
          <span>Local source search</span>
        </div>
      </div>

      <nav className="nav-list" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          const badge =
            item.key === "sources"
              ? sourcesCount
              : item.key === "favorites"
                ? favoritesCount
                : item.key === "history"
                  ? historyCount
                  : null;

          return (
            <button
              key={item.key}
              className={cx("nav-item", activeView === item.key && "is-active")}
              type="button"
              onClick={() => onViewChange(item.key)}
              title={item.label}
            >
              <Icon size={18} />
              <span>{item.label}</span>
              {badge !== null && <em>{badge}</em>}
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
