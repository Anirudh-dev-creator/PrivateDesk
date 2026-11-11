import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { baseName, errText, folderOf, formatBytes, useFileDrop } from "../lib/helpers";

type Item = {
  path: string;
  name: string;
  status: "waiting" | "working" | "done" | "failed";
  message: string;
  originalBytes?: number;
  outputBytes?: number;
  outputPath?: string;
};

type Result = { originalBytes: number; outputBytes: number; outputPath: string; note: string };

const UNITS = { KB: 1000, KiB: 1024, MB: 1_000_000, MiB: 1_048_576 } as const;
type Unit = keyof typeof UNITS;
const QUICK: { label: string; value: number; unit: Unit }[] = [
  { label: "20 KB", value: 20, unit: "KB" },
  { label: "50 KB", value: 50, unit: "KB" },
  { label: "100 KB", value: 100, unit: "KB" },
  { label: "200 KB", value: 200, unit: "KB" },
  { label: "500 KB", value: 500, unit: "KB" },
  { label: "1 MB", value: 1, unit: "MB" },
];
const EXTS = ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff", "pdf"];

function csvCell(text: string | number | undefined): string {
  return `"${String(text ?? "").replace(/"/g, '""')}"`;
}

export default function BatchCompress() {
  const [items, setItems] = useState<Item[]>([]);
  const [outDir, setOutDir] = useState("");
  const [value, setValue] = useState("100");
  const [unit, setUnit] = useState<Unit>("KB");
  const [mode, setMode] = useState<"maximum" | "closest">("maximum");
  const [allowResize, setAllowResize] = useState(true);
  const [keepAlpha, setKeepAlpha] = useState(true);
  const [suffix, setSuffix] = useState("_compressed");
  const [running, setRunning] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelRef = useRef(false);

  const targetBytes = Math.floor((Number(value) || 0) * UNITS[unit]);

  const addPaths = (paths: string[]) => {
    setItems((old) => {
      const known = new Set(old.map((i) => i.path));
      const fresh = paths
        .filter((p) => !known.has(p) && EXTS.includes((p.split(".").pop() ?? "").toLowerCase()))
        .map<Item>((p) => ({ path: p, name: baseName(p), status: "waiting", message: "" }));
      return [...old, ...fresh];
    });
    setOutDir((d) => d || (paths[0] ? folderOf(paths[0]) : d));
  };
  useFileDrop(addPaths, setDragging);

  const chooseFiles = async () => {
    const picked = await open({ multiple: true, directory: false, filters: [{ name: "Images and PDFs", extensions: EXTS }] });
    if (Array.isArray(picked)) addPaths(picked);
    else if (typeof picked === "string") addPaths([picked]);
  };

  const chooseFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setOutDir(picked);
  };

  const patch = (path: string, change: Partial<Item>) =>
    setItems((old) => old.map((i) => (i.path === path ? { ...i, ...change } : i)));

  const run = async () => {
    if (targetBytes < 1000) {
      setError("Enter a target of at least 1 KB (1,000 bytes).");
      return;
    }
    if (!outDir) {
      setError("Choose an output folder first.");
      return;
    }
    setError(null);
    setRunning(true);
    cancelRef.current = false;
    const todo = items.filter((i) => i.status !== "done");
    for (const item of todo) {
      if (cancelRef.current) break;
      patch(item.path, { status: "working", message: "" });
      try {
        const r = await invoke<Result>("compress_into_folder", {
          request: {
            inputPath: item.path,
            outputDir: outDir,
            suffix,
            targetBytes,
            outputFormat: "auto",
            mode,
            allowResize,
            preserveTransparency: keepAlpha,
            minQuality: 20,
          },
        });
        patch(item.path, {
          status: "done",
          message: r.note,
          originalBytes: r.originalBytes,
          outputBytes: r.outputBytes,
          outputPath: r.outputPath,
        });
      } catch (e) {
        patch(item.path, { status: "failed", message: errText(e) });
      }
    }
    setRunning(false);
  };

  const cancel = () => {
    cancelRef.current = true;
    void invoke("cancel_compression");
  };

  const exportReport = async () => {
    const lines = ["file,status,original_bytes,output_bytes,limit_bytes,output_path,message"];
    for (const i of items) {
      lines.push([csvCell(i.name), csvCell(i.status), csvCell(i.originalBytes), csvCell(i.outputBytes), csvCell(targetBytes), csvCell(i.outputPath), csvCell(i.message)].join(","));
    }
    const path = await save({ defaultPath: `${outDir || "."}\\compression_report.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      await invoke("save_text_file", { path, content: lines.join("\r\n") });
    } catch (e) {
      setError(errText(e));
    }
  };

  const done = items.filter((i) => i.status === "done").length;
  const failed = items.filter((i) => i.status === "failed").length;

  return (
    <div className="pd-page">
      <h1>Batch Compress</h1>
      <p className="pd-lead">Compress many images and PDFs to the same size limit. Originals are never changed; results go to the folder you choose.</p>

      <div className={`pd-drop ${dragging ? "drag" : ""}`} onClick={() => !running && void chooseFiles()} role="button" tabIndex={0}
        onKeyDown={(e) => { if (e.key === "Enter") void chooseFiles(); }}>
        <b>Drop files here or click to add files</b>
        <small>JPG, PNG, WebP, BMP, TIFF and PDF</small>
      </div>

      <div className="pd-panel">
        <h2>Size limit</h2>
        <div className="pd-row">
          {QUICK.map((q) => (
            <button key={q.label} type="button" className={`pd-chip ${targetBytes === q.value * UNITS[q.unit] ? "active" : ""}`}
              onClick={() => { setValue(String(q.value)); setUnit(q.unit); }}>{q.label}</button>
          ))}
        </div>
        <div className="pd-row">
          <input className="pd-input" type="number" min="1" step="any" value={value} onChange={(e) => setValue(e.target.value)} aria-label="Target size" style={{ width: 110 }} />
          <select className="pd-input" value={unit} onChange={(e) => setUnit(e.target.value as Unit)} aria-label="Unit">
            <option value="KB">KB (1,000 bytes)</option><option value="KiB">KiB (1,024 bytes)</option>
            <option value="MB">MB (1,000,000 bytes)</option><option value="MiB">MiB (1,048,576 bytes)</option>
          </select>
          <span className="pd-cap">Exact limit: {targetBytes.toLocaleString()} bytes</span>
        </div>
        <div className="pd-row">
          <button type="button" className={`pd-chip ${mode === "maximum" ? "active" : ""}`} onClick={() => setMode("maximum")}>Maximum size</button>
          <button type="button" className={`pd-chip ${mode === "closest" ? "active" : ""}`} onClick={() => setMode("closest")}>Closest possible size</button>
        </div>
        <label className="pd-check"><input type="checkbox" checked={allowResize} onChange={(e) => setAllowResize(e.target.checked)} /> Allow resolution reduction</label>
        <label className="pd-check"><input type="checkbox" checked={keepAlpha} onChange={(e) => setKeepAlpha(e.target.checked)} /> Keep transparency (transparent images stay PNG)</label>
        <div className="pd-row">
          <label className="pd-field" style={{ margin: 0 }}>File name suffix
            <input type="text" value={suffix} onChange={(e) => setSuffix(e.target.value)} />
          </label>
          <div style={{ flex: 1 }} />
          <button type="button" className="pd-btn secondary" onClick={() => void chooseFolder()}>Choose output folder</button>
        </div>
        <div className="pd-cap">Output folder: {outDir || "not chosen yet"}</div>
      </div>

      {error && <div className="pd-error" role="alert">{error}</div>}

      <div className="pd-panel">
        <div className="pd-row">
          <h2 style={{ margin: 0 }}>Files ({items.length})</h2>
          <span className="pd-cap">{done} done · {failed} failed</span>
          <div style={{ flex: 1 }} />
          <button type="button" className="pd-btn secondary small" disabled={running || items.length === 0} onClick={() => setItems([])}>Clear list</button>
          <button type="button" className="pd-btn secondary small" disabled={items.length === 0} onClick={() => void exportReport()}>Export report (CSV)</button>
          {running && <button type="button" className="pd-btn secondary" onClick={cancel}>Cancel</button>}
          <button type="button" className="pd-btn" disabled={running || items.length === 0 || done === items.length} onClick={() => void run()}>
            {running ? "Compressing…" : failed > 0 ? "Start / retry failed" : "Start"}
          </button>
        </div>
        {items.length === 0 ? <p className="pd-cap">No files yet.</p> : (
          <table className="pd-table">
            <thead><tr><th>File</th><th>Status</th><th>Result</th></tr></thead>
            <tbody>
              {items.map((i) => (
                <tr key={i.path}>
                  <td>{i.name}</td>
                  <td className={i.status === "done" ? "pd-ok-text" : i.status === "failed" ? "pd-bad-text" : ""}>{i.status}</td>
                  <td>
                    {i.status === "done" && i.outputBytes !== undefined
                      ? `${formatBytes(i.originalBytes ?? 0)} → ${formatBytes(i.outputBytes)}`
                      : i.message}
                    {i.status === "done" && i.outputPath && (
                      <> <button type="button" className="pd-btn secondary small" onClick={() => void invoke("reveal_in_folder", { path: i.outputPath })}>Show</button></>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
