import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TargetSizeCompressor } from "./features/compressor";
import "./app.css";

type Page = "home" | "compressor";
type Theme = "system" | "light" | "dark";

const PLANNED = [
  "PDF Tools",
  "Document Editor",
  "File Converter",
  "Image Studio",
  "Passport & ID Photos",
  "OCR & Scanner",
  "Batch Processing",
  "File Manager",
  "Security & Privacy",
];

function loadTheme(): Theme {
  try {
    const saved = localStorage.getItem("privatedesk-theme");
    if (saved === "light" || saved === "dark" || saved === "system") return saved;
  } catch { /* storage unavailable */ }
  return "system";
}

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [theme, setTheme] = useState<Theme>(loadTheme);
  const [ghostscript, setGhostscript] = useState<boolean | null>(null);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
    try { localStorage.setItem("privatedesk-theme", theme); } catch { /* ignore */ }
  }, [theme]);

  useEffect(() => {
    invoke<{ ghostscript: boolean }>("engine_status")
      .then((s) => setGhostscript(s.ghostscript))
      .catch(() => setGhostscript(false));
  }, []);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">PrivateDesk</div>
        <nav className="nav" aria-label="Main navigation">
          <button className={page === "home" ? "active" : ""} onClick={() => setPage("home")}>Home</button>
          <button className={page === "compressor" ? "active" : ""} onClick={() => setPage("compressor")}>Target Size Compressor</button>
          <small>PLANNED – NOT YET BUILT</small>
          {PLANNED.map((name) => (
            <button key={name} className="planned" disabled aria-disabled="true" title="Not implemented yet">{name}</button>
          ))}
        </nav>
        <div className="sidebar-foot">
          <label htmlFor="theme">Theme</label>
          <select id="theme" value={theme} onChange={(e) => setTheme(e.target.value as Theme)}>
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
      </aside>

      <main className="content">
        {page === "home" && (
          <div className="home">
            <h1>PrivateDesk</h1>
            <p className="lead">Document and file tools that run entirely on this PC. Nothing is uploaded.</p>
            <div className="cards">
              <div className="card">
                <h3>Target Size Compressor</h3>
                <p>Compress a photo or PDF to a maximum size such as 100 KB. The saved file is measured to confirm it fits.</p>
                <button onClick={() => setPage("compressor")}>Open</button>
              </div>
            </div>
            <div className="card">
              <h3>Status</h3>
              <div className="status-list">
                <div className="status-row"><span className="dot" /> Offline: this app contains no network features for your files.</div>
                <div className="status-row"><span className="dot" /> Image compression engine: built in.</div>
                <div className="status-row">
                  <span className={`dot ${ghostscript ? "" : "off"}`} />
                  PDF engine (Ghostscript): {ghostscript === null ? "checking…" : ghostscript ? "found" : "not found. Install Ghostscript to compress PDFs."}
                </div>
              </div>
            </div>
          </div>
        )}
        {page === "compressor" && <TargetSizeCompressor />}
      </main>
    </div>
  );
}
