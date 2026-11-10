import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open, save } from "@tauri-apps/plugin-dialog";
import "./target-size-compressor.css";

type FileInfo = {
  path: string;
  name: string;
  sizeBytes: number;
  extension: string;
  isPdf: boolean;
  width: number | null;
  height: number | null;
  hasAlpha: boolean | null;
};

type CompressionResult = {
  inputPath: string;
  outputPath: string;
  originalBytes: number;
  outputBytes: number;
  targetBytes: number;
  reductionPercent: number;
  outputFormat: string;
  width: number | null;
  height: number | null;
  quality: number | null;
  mode: string;
  metTarget: boolean;
  note: string;
};

type OutputFormat = "auto" | "jpeg" | "png";
type Mode = "maximum" | "closest";

// 1 KB = 1,000 bytes, 1 KiB = 1,024 bytes, 1 MB = 1,000,000 bytes, 1 MiB = 1,048,576 bytes.
const UNITS = { KB: 1000, KiB: 1024, MB: 1_000_000, MiB: 1_048_576 } as const;
type SizeUnit = keyof typeof UNITS;

const PRESETS: { label: string; value: number; unit: SizeUnit }[] = [
  { label: "10 KB", value: 10, unit: "KB" },
  { label: "20 KB", value: 20, unit: "KB" },
  { label: "30 KB", value: 30, unit: "KB" },
  { label: "50 KB", value: 50, unit: "KB" },
  { label: "80 KB", value: 80, unit: "KB" },
  { label: "100 KB", value: 100, unit: "KB" },
  { label: "150 KB", value: 150, unit: "KB" },
  { label: "200 KB", value: 200, unit: "KB" },
  { label: "300 KB", value: 300, unit: "KB" },
  { label: "500 KB", value: 500, unit: "KB" },
  { label: "800 KB", value: 800, unit: "KB" },
  { label: "1 MB", value: 1, unit: "MB" },
  { label: "2 MB", value: 2, unit: "MB" },
  { label: "5 MB", value: 5, unit: "MB" },
  { label: "10 MB", value: 10, unit: "MB" },
];

const MIN_TARGET_BYTES = 1000;

function formatBytes(bytes: number): string {
  if (bytes < 1000) return `${bytes.toLocaleString()} bytes`;
  if (bytes < 1_000_000) return `${(bytes / 1000).toFixed(1)} KB (${bytes.toLocaleString()} bytes)`;
  return `${(bytes / 1_000_000).toFixed(2)} MB (${bytes.toLocaleString()} bytes)`;
}

function extensionFor(info: FileInfo, format: OutputFormat, targetBytes: number, preserveTransparency: boolean): string {
  if (info.isPdf) return "pdf";
  if (format === "png") return "png";
  if (format === "jpeg") return "jpg";
  if (info.sizeBytes <= targetBytes) return info.extension || "bin";
  if (info.hasAlpha && preserveTransparency) return "png";
  return "jpg";
}

function outputName(info: FileInfo, extension: string): string {
  const stem = info.name.replace(/\.[^.]+$/, "");
  return `${stem}_compressed.${extension}`;
}

export default function TargetSizeCompressor() {
  const [file, setFile] = useState<FileInfo | null>(null);
  // Default: 100 KB maximum = 100,000 bytes.
  const [targetValue, setTargetValue] = useState("100");
  const [unit, setUnit] = useState<SizeUnit>("KB");
  const [mode, setMode] = useState<Mode>("maximum");
  const [format, setFormat] = useState<OutputFormat>("auto");
  const [allowResize, setAllowResize] = useState(true);
  const [preserveTransparency, setPreserveTransparency] = useState(true);
  const [minQuality, setMinQuality] = useState(20);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CompressionResult | null>(null);

  const targetBytes = useMemo(() => {
    const value = Number(targetValue);
    if (!Number.isFinite(value) || value <= 0) return 0;
    return Math.floor(value * UNITS[unit]);
  }, [targetValue, unit]);

  const loadPath = useCallback(async (path: string) => {
    setError(null);
    setResult(null);
    try {
      const info = await invoke<FileInfo>("inspect_compressor_file", { inputPath: path });
      setFile(info);
      if (info.isPdf) setFormat("auto");
    } catch (e) {
      setFile(null);
      setError(String(e));
    }
  }, []);

  const chooseFile = useCallback(async () => {
    setError(null);
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Images and PDFs", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "pdf"] }],
    });
    if (typeof selected === "string") await loadPath(selected);
  }, [loadPath]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") setDragging(true);
        if (event.payload.type === "leave") setDragging(false);
        if (event.payload.type === "drop") {
          setDragging(false);
          const firstPath = event.payload.paths?.[0];
          if (firstPath) void loadPath(firstPath);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((e) => setError(`File drop is unavailable: ${String(e)}`));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [loadPath]);

  const targetTooSmall = targetBytes < MIN_TARGET_BYTES;

  const runCompression = useCallback(async () => {
    if (!file) return;
    if (targetTooSmall) {
      setError("Enter a target of at least 1 KB (1,000 bytes).");
      return;
    }
    const ext = extensionFor(file, format, targetBytes, preserveTransparency);
    const folder = file.path.replace(/[\\/][^\\/]*$/, "");
    const defaultPath = `${folder}\\${outputName(file, ext)}`;
    const outputPath = await save({
      defaultPath,
      filters: [{ name: `${ext.toUpperCase()} file`, extensions: [ext] }],
    });
    if (!outputPath) return;

    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const completed = await invoke<CompressionResult>("compress_to_target", {
        request: {
          inputPath: file.path,
          outputPath,
          targetBytes,
          outputFormat: format,
          mode,
          allowResize,
          preserveTransparency,
          minQuality,
        },
      });
      setResult(completed);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [file, targetBytes, targetTooSmall, format, mode, allowResize, preserveTransparency, minQuality]);

  const cancel = () => {
    void invoke("cancel_compression");
  };

  const reveal = (path: string) => {
    invoke("reveal_in_folder", { path }).catch((e) => setError(String(e)));
  };

  const choosePreset = (value: number, presetUnit: SizeUnit) => {
    setTargetValue(String(value));
    setUnit(presetUnit);
    setResult(null);
  };

  return (
    <div className="tsc-page">
      <header className="tsc-header">
        <div className="tsc-heading-icon" aria-hidden="true">⇩</div>
        <div>
          <p className="tsc-eyebrow">PRIVATEDESK · LOCAL TOOLS</p>
          <h1>Target Size Compressor</h1>
          <p className="tsc-subtitle">Compress images and PDFs to a size limit. Your files stay on this PC.</p>
        </div>
        <span className="tsc-local-pill"><span /> Offline processing</span>
      </header>

      <section
        className={`tsc-dropzone ${dragging ? "is-dragging" : ""} ${file ? "has-file" : ""}`}
        onClick={() => !busy && void chooseFile()}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            void chooseFile();
          }
        }}
        aria-label="Choose or drop an image or PDF"
      >
        <div className="tsc-drop-icon">{file?.isPdf ? "PDF" : "＋"}</div>
        {file ? (
          <>
            <strong>{file.name}</strong>
            <span>{formatBytes(file.sizeBytes)}{file.width && file.height ? ` · ${file.width} × ${file.height}px` : ""}</span>
            <small>Click to choose a different file, or drop another file here</small>
          </>
        ) : (
          <>
            <strong>Drop a file here or click to browse</strong>
            <span>JPG, PNG, WebP, BMP, TIFF and PDF · images up to 24 megapixels</span>
            <small>Everything is processed locally; nothing is uploaded.</small>
          </>
        )}
      </section>

      <section className="tsc-panel">
        <div className="tsc-section-title">
          <div><h2>Size limit</h2><p>Default: 100 KB maximum (100,000 bytes).</p></div>
        </div>

        <div className="tsc-modes" role="radiogroup" aria-label="Compression mode">
          <button type="button" role="radio" aria-checked={mode === "maximum"} className={`tsc-mode ${mode === "maximum" ? "active" : ""}`} onClick={() => setMode("maximum")}>
            <b>Maximum size</b><small>Highest quality that fits at or under the limit.</small>
          </button>
          <button type="button" role="radio" aria-checked={mode === "closest"} className={`tsc-mode ${mode === "closest" ? "active" : ""}`} onClick={() => setMode("closest")}>
            <b>Closest possible size</b><small>Gets as near the limit as it can (never above it). May use a lower quality to do so.</small>
          </button>
        </div>

        <div className="tsc-preset-grid">
          {PRESETS.map((p) => (
            <button
              type="button"
              key={p.label}
              className={`tsc-preset ${targetBytes === p.value * UNITS[p.unit] ? "active" : ""}`}
              onClick={() => choosePreset(p.value, p.unit)}
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="tsc-custom-row">
          <label htmlFor="tsc-target">Custom target</label>
          <div className="tsc-size-input">
            <input
              id="tsc-target"
              type="number"
              min="1"
              step="any"
              value={targetValue}
              onChange={(e) => { setTargetValue(e.target.value); setResult(null); }}
            />
            <select value={unit} onChange={(e) => { setUnit(e.target.value as SizeUnit); setResult(null); }} aria-label="Target unit">
              <option value="KB">KB (1,000 bytes)</option>
              <option value="KiB">KiB (1,024 bytes)</option>
              <option value="MB">MB (1,000,000 bytes)</option>
              <option value="MiB">MiB (1,048,576 bytes)</option>
            </select>
          </div>
          <span className="tsc-byte-equivalent">
            {targetBytes ? `Exact limit: ${targetBytes.toLocaleString()} bytes` : "Enter a valid size"}
          </span>
        </div>
      </section>

      <section className="tsc-panel">
        <div className="tsc-section-title"><div><h2>Output and quality</h2><p>Choose how to balance quality and size.</p></div></div>
        <div className="tsc-settings-grid">
          <div>
            <label className="tsc-field"><span>Output format</span>
              <select value={file?.isPdf ? "auto" : format} disabled={!file || file.isPdf} onChange={(e) => setFormat(e.target.value as OutputFormat)}>
                <option value="auto">Auto (best fit)</option>
                <option value="jpeg">JPEG (smallest photos)</option>
                <option value="png">PNG (lossless, may not meet small limits)</option>
              </select>
            </label>
            <div className="tsc-slider">
              <label htmlFor="tsc-quality" className="tsc-field"><span>Lowest JPEG quality allowed: {minQuality}</span></label>
              <input id="tsc-quality" type="range" min={10} max={90} step={5} value={minQuality} onChange={(e) => setMinQuality(Number(e.target.value))} />
              <small>Higher keeps pictures cleaner but may make a small limit impossible.</small>
            </div>
          </div>
          <div className="tsc-toggle-list">
            <label className="tsc-toggle">
              <input type="checkbox" checked={allowResize} onChange={(e) => setAllowResize(e.target.checked)} />
              <span><b>Allow resolution reduction</b><small>Shrink the pixel dimensions if quality changes alone cannot reach the limit.</small></span>
            </label>
            <label className="tsc-toggle">
              <input type="checkbox" checked={preserveTransparency} onChange={(e) => setPreserveTransparency(e.target.checked)} />
              <span><b>Keep transparency in Auto mode</b><small>Transparent images stay PNG. JPEG cannot be transparent and fills transparent areas with white.</small></span>
            </label>
          </div>
        </div>
        <div className="tsc-info-note">
          <b>How it works:</b> the compressor creates real output, measures its size, and saves only a result at or under your limit. It never pads a file. If the limit cannot be reached with acceptable quality, it says so and saves nothing.
        </div>
      </section>

      {file?.isPdf && (
        <div className="tsc-alert warn" role="note">
          <b>PDF compression uses Ghostscript</b>
          <span>PDFs are compressed by the free Ghostscript program, which must be installed on this PC. Scanned or image-heavy PDFs compress best.</span>
        </div>
      )}

      {error && <div className="tsc-alert error" role="alert"><b>Could not compress</b><span>{error}</span></div>}

      {result && (
        <section className="tsc-result" aria-live="polite">
          <div className="tsc-result-top">
            <div className="tsc-success-mark">✓</div>
            <div>
              <h2>Compression complete</h2>
              <p>{result.metTarget ? "Verified on disk: the saved file is within your limit." : "The output did not meet the selected limit."}</p>
            </div>
            <strong className="tsc-output-size">{result.outputBytes.toLocaleString()} bytes</strong>
          </div>
          <div className="tsc-stats">
            <div><small>Original</small><b>{formatBytes(result.originalBytes)}</b></div>
            <div><small>Final size</small><b>{formatBytes(result.outputBytes)}</b></div>
            <div><small>Limit</small><b>{result.targetBytes.toLocaleString()} bytes</b></div>
            <div><small>Reduced by</small><b>{result.reductionPercent.toFixed(1)}%</b></div>
          </div>
          <p className="tsc-result-dimensions">
            Format: {result.outputFormat.toUpperCase()}
            {result.width && result.height ? ` · ${result.width} × ${result.height}px` : ""}
            {result.quality ? ` · JPEG quality ${result.quality}` : ""}
            {` · Mode: ${result.mode === "closest" ? "closest size" : "maximum size"}`}
          </p>
          <p className="tsc-result-note">{result.note}</p>
          <p className="tsc-saved-path">Saved: {result.outputPath}</p>
          <div className="tsc-result-actions">
            <button type="button" className="tsc-secondary" onClick={() => reveal(result.outputPath)}>Show in folder</button>
          </div>
        </section>
      )}

      <footer className="tsc-footer">
        <div><b>Original files are never overwritten.</b><span>Choose a new file name in the Save dialog.</span></div>
        <div className="tsc-footer-actions">
          {busy && <button type="button" className="tsc-secondary" onClick={cancel}>Cancel</button>}
          <button type="button" className="tsc-primary" disabled={!file || busy || targetTooSmall} onClick={() => void runCompression()}>
            {busy ? <><span className="tsc-spinner" /> Compressing locally…</> : mode === "maximum" ? "Compress to maximum size" : "Compress to closest size"}
          </button>
        </div>
      </footer>
    </div>
  );
}
