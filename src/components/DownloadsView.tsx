import { CircleSlash, Download } from "lucide-react";

export function DownloadsView() {
  return (
    <section className="view">
      <header className="view-header">
        <div>
          <h1>Downloads</h1>
          <p>0 active</p>
        </div>
      </header>

      <div className="downloads-table">
        <div className="downloads-head">
          <span>Filename</span>
          <span>Source</span>
          <span>Progress</span>
          <span>Status</span>
        </div>
        <div className="empty-panel table-empty">
          <Download size={20} />
          <strong>No downloads</strong>
          <span>Direct downloadable video files will appear here.</span>
        </div>
      </div>
    </section>
  );
}
