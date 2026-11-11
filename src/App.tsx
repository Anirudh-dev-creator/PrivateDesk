import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TargetSizeCompressor } from "./features/compressor";
import BatchCompress from "./pages/BatchCompress";
import ImageStudio from "./pages/ImageStudio";
import PassportStudio from "./pages/PassportStudio";
import Utilities from "./pages/Utilities";
import "./app.css";

type Page = "home" | "compressor" | "batch" | "studio" | "passport" | "utilities";
type Theme = "system" | "light" | "dark";

const TOOLS: { id: Page; label: string; blurb: string }[] = [
  { id: "compressor", label: "Target Size Compressor", blurb: "Compress a photo or PDF to a size limit such as 100 KB." },
  { id: "batch", label: "Batch Compress", blurb: "Compress many files at once and export a report." },
  { id: "studio", label: "Image Studio", blurb: "Resize, crop, rotate, adjust and convert images; view EXIF." },
  { id: "passport", label: "Passport & ID Photos", blurb: "Exact-size photos and print sheets (manual positioning)." },
  { id: "utilities", label: "Utilities", blurb: "File hash, QR code, Base64, text tools, JSON, duplicates, bulk rename." },
];

const PLANNED = [
  "PDF Tools",
  "Document Editor",
  "File Converter",
  "OCR & Scanner",
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
          {TOOLS.map((t) => (
            <button key={t.id} className={page === t.id ? "active" : ""} onClick={() => setPage(t.id)}>{t.label}</button>
          ))}
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
              {TOOLS.map((t) => (
                <div className="card" key={t.id}>
                  <h3>{t.label}</h3>
                  <p>{t.blurb}</p>
                  <button onClick={() => setPage(t.id)}>Open</button>
                </div>
              ))}
            </div>
            <div className="card">
              <h3>Status</h3>
              <div className="status-list">
                <div className="status-row"><span className="dot" /> Offline: this app has no network features for your files.</div>
                <div className="status-row"><span className="dot" /> Image engine: built in.</div>
                <div className="status-row">
                  <span className={`dot ${ghostscript ? "" : "off"}`} />
                  PDF compression engine (Ghostscript): {ghostscript === null ? "checking…" : ghostscript ? "found" : "not found. Install Ghostscript to compress PDFs."}
                </div>
              </div>
            </div>
          </div>
        )}
        {page === "compressor" && <TargetSizeCompressor />}
        {page === "batch" && <BatchCompress />}
        {page === "studio" && <ImageStudio />}
        {page === "passport" && <PassportStudio />}
        {page === "utilities" && <Utilities />}
      </main>
    </div>
  );
}
