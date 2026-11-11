import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { baseName, errText, formatBytes, useFileDrop } from "../lib/helpers";

type Tab = "hash" | "qr" | "base64" | "text" | "json" | "duplicates" | "rename";
const TABS: { id: Tab; label: string }[] = [
  { id: "hash", label: "File hash" },
  { id: "qr", label: "QR code" },
  { id: "base64", label: "Base64" },
  { id: "text", label: "Text tools" },
  { id: "json", label: "JSON" },
  { id: "duplicates", label: "Duplicate finder" },
  { id: "rename", label: "Bulk rename" },
];

function copy(text: string) {
  void navigator.clipboard.writeText(text).catch(() => undefined);
}

// ---------------- File hash ----------------
type HashInfo = { path: string; name: string; sizeBytes: number; sha256: string; md5: string };

function HashTab() {
  const [info, setInfo] = useState<HashInfo | null>(null);
  const [expected, setExpected] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = async (path: string) => {
    setBusy(true);
    setError(null);
    try {
      setInfo(await invoke<HashInfo>("hash_file", { path }));
    } catch (e) {
      setInfo(null);
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };
  useFileDrop((p) => void load(p[0]));

  const choose = async () => {
    const picked = await open({ multiple: false, directory: false });
    if (typeof picked === "string") await load(picked);
  };

  const want = expected.trim().toLowerCase();
  const matches = info && want ? (want === info.sha256 ? "SHA-256" : want === info.md5 ? "MD5" : "none") : null;

  return (
    <div className="pd-panel">
      <div className="pd-row">
        <button type="button" className="pd-btn" disabled={busy} onClick={() => void choose()}>Choose a file</button>
        <span className="pd-cap">or drop a file on the window</span>
      </div>
      {error && <div className="pd-error">{error}</div>}
      {busy && <p className="pd-cap">Calculating…</p>}
      {info && (
        <>
          <p className="pd-cap">{info.name} · {formatBytes(info.sizeBytes)}</p>
          <p><b>SHA-256</b> <button type="button" className="pd-btn secondary small" onClick={() => copy(info.sha256)}>Copy</button><br /><span className="pd-mono">{info.sha256}</span></p>
          <p><b>MD5</b> <button type="button" className="pd-btn secondary small" onClick={() => copy(info.md5)}>Copy</button><br /><span className="pd-mono">{info.md5}</span></p>
          <label className="pd-field">Compare with a hash you were given
            <input type="text" value={expected} onChange={(e) => setExpected(e.target.value)} placeholder="Paste the expected SHA-256 or MD5" />
          </label>
          {matches === "none" && <div className="pd-error">No match. The file is different from what that hash describes.</div>}
          {matches && matches !== "none" && <div className="pd-ok">Match ({matches}). The file is identical.</div>}
        </>
      )}
    </div>
  );
}

// ---------------- QR ----------------
function QrTab() {
  const [text, setText] = useState("https://");
  const [size, setSize] = useState(8);
  const [img, setImg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(async () => {
      if (!text.trim()) { setImg(null); setError(null); return; }
      try {
        const r = await invoke<{ dataUrl: string }>("qr_preview", { text, modulePx: size });
        setImg(r.dataUrl);
        setError(null);
      } catch (e) {
        setImg(null);
        setError(errText(e));
      }
    }, 250);
    return () => window.clearTimeout(timer);
  }, [text, size]);

  const saveQr = async () => {
    const out = await save({ defaultPath: "qrcode.png", filters: [{ name: "PNG", extensions: ["png"] }] });
    if (!out) return;
    try {
      await invoke("qr_save", { text, modulePx: size, outputPath: out });
      setSaved(out);
      setError(null);
    } catch (e) {
      setError(errText(e));
    }
  };

  return (
    <div className="pd-panel">
      <label className="pd-field">Text or link<textarea value={text} onChange={(e) => setText(e.target.value)} style={{ minHeight: 80 }} /></label>
      <label className="pd-field">Size: {size} pixels per square<input type="range" min={4} max={20} value={size} onChange={(e) => setSize(Number(e.target.value))} /></label>
      {error && <div className="pd-error">{error}</div>}
      {saved && <div className="pd-ok">Saved: {saved}</div>}
      <div className="pd-preview" style={{ background: "#fff", minHeight: 200 }}>{img && <img src={img} alt="QR code" style={{ imageRendering: "pixelated" }} />}</div>
      <div className="pd-row" style={{ marginTop: 10 }}>
        <button type="button" className="pd-btn" disabled={!img} onClick={() => void saveQr()}>Save as PNG</button>
      </div>
    </div>
  );
}

// ---------------- Base64 ----------------
function Base64Tab() {
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const [error, setError] = useState<string | null>(null);

  const encode = async () => {
    setError(null);
    setOutput(await invoke<string>("b64_encode", { text: input }));
  };
  const decode = async () => {
    setError(null);
    try {
      setOutput(await invoke<string>("b64_decode", { text: input }));
    } catch (e) {
      setOutput("");
      setError(errText(e));
    }
  };

  return (
    <div className="pd-panel">
      <label className="pd-field">Input<textarea value={input} onChange={(e) => setInput(e.target.value)} /></label>
      <div className="pd-row">
        <button type="button" className="pd-btn" onClick={() => void encode()}>Encode</button>
        <button type="button" className="pd-btn secondary" onClick={() => void decode()}>Decode</button>
        <button type="button" className="pd-btn secondary" disabled={!output} onClick={() => copy(output)}>Copy result</button>
      </div>
      {error && <div className="pd-error">{error}</div>}
      <label className="pd-field">Result<textarea value={output} readOnly /></label>
    </div>
  );
}

// ---------------- Text tools ----------------
function uniq(list: string[]): string[] {
  return Array.from(new Set(list));
}

function TextTab() {
  const [text, setText] = useState("");
  const [out, setOut] = useState("");

  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  const lines = text ? text.split(/\r?\n/).length : 0;

  const run = (fn: (t: string) => string) => setOut(fn(text));
  const title = (t: string) => t.toLowerCase().replace(/(^|\s)(\S)/g, (_m, a: string, b: string) => a + b.toUpperCase());
  const actions: { label: string; fn: (t: string) => string }[] = [
    { label: "UPPERCASE", fn: (t) => t.toUpperCase() },
    { label: "lowercase", fn: (t) => t.toLowerCase() },
    { label: "Title Case", fn: title },
    { label: "Remove extra spaces", fn: (t) => t.split(/\r?\n/).map((l) => l.replace(/\s+/g, " ").trim()).join("\n") },
    { label: "Remove blank lines", fn: (t) => t.split(/\r?\n/).filter((l) => l.trim() !== "").join("\n") },
    { label: "Remove duplicate lines", fn: (t) => uniq(t.split(/\r?\n/)).join("\n") },
    { label: "Sort lines A–Z", fn: (t) => t.split(/\r?\n/).sort((a, b) => a.localeCompare(b)).join("\n") },
    { label: "Extract emails", fn: (t) => uniq(t.match(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi) ?? []).join("\n") },
    { label: "Extract links", fn: (t) => uniq(t.match(/https?:\/\/[^\s<>"')]+/gi) ?? []).join("\n") },
    { label: "Extract phone numbers (approx.)", fn: (t) => uniq(t.match(/(?:\+\d{1,3}[\s-]?)?(?:\(?\d{2,5}\)?[\s-]?)?\d{3,4}[\s-]?\d{3,4}/g) ?? []).join("\n") },
  ];

  return (
    <div className="pd-panel">
      <label className="pd-field">Your text<textarea value={text} onChange={(e) => setText(e.target.value)} /></label>
      <p className="pd-cap">{words} words · {text.length} characters · {lines} lines</p>
      <div className="pd-row">
        {actions.map((a) => <button key={a.label} type="button" className="pd-chip" onClick={() => run(a.fn)}>{a.label}</button>)}
      </div>
      <label className="pd-field">Result<textarea value={out} readOnly /></label>
      <div className="pd-row">
        <button type="button" className="pd-btn secondary" disabled={!out} onClick={() => copy(out)}>Copy result</button>
        <button type="button" className="pd-btn secondary" disabled={!out} onClick={() => setText(out)}>Use result as input</button>
      </div>
    </div>
  );
}

// ---------------- JSON ----------------
function JsonTab() {
  const [text, setText] = useState("");
  const [out, setOut] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  const work = (mode: "format" | "minify" | "validate") => {
    try {
      const parsed: unknown = JSON.parse(text);
      if (mode === "format") setOut(JSON.stringify(parsed, null, 2));
      if (mode === "minify") setOut(JSON.stringify(parsed));
      setMsg({ ok: true, text: "Valid JSON." });
    } catch (e) {
      setMsg({ ok: false, text: errText(e) });
    }
  };

  return (
    <div className="pd-panel">
      <label className="pd-field">JSON<textarea value={text} onChange={(e) => setText(e.target.value)} /></label>
      <div className="pd-row">
        <button type="button" className="pd-btn" onClick={() => work("format")}>Format</button>
        <button type="button" className="pd-btn secondary" onClick={() => work("minify")}>Minify</button>
        <button type="button" className="pd-btn secondary" onClick={() => work("validate")}>Validate</button>
        <button type="button" className="pd-btn secondary" disabled={!out} onClick={() => copy(out)}>Copy result</button>
      </div>
      {msg && <div className={msg.ok ? "pd-ok" : "pd-error"}>{msg.text}</div>}
      <label className="pd-field">Result<textarea value={out} readOnly /></label>
    </div>
  );
}

// ---------------- Duplicate finder ----------------
type Group = { sizeBytes: number; sha256: string; files: string[] };
type DupReport = { groups: Group[]; filesScanned: number; wastedBytes: number; truncated: boolean };

function DuplicatesTab() {
  const [folder, setFolder] = useState("");
  const [minKb, setMinKb] = useState("1");
  const [report, setReport] = useState<DupReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scan = async (dir: string) => {
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      setReport(await invoke<DupReport>("find_duplicates", { folder: dir, minSize: Math.floor((Number(minKb) || 1) * 1000) }));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const choose = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") {
      setFolder(picked);
      await scan(picked);
    }
  };

  return (
    <div className="pd-panel">
      <p className="pd-cap">Finds files with identical content, including subfolders. It only reports; it never deletes or moves anything.</p>
      <div className="pd-row">
        <button type="button" className="pd-btn" disabled={busy} onClick={() => void choose()}>Choose a folder and scan</button>
        <label className="pd-field" style={{ margin: 0, flexDirection: "row", alignItems: "center" }}>Ignore files smaller than (KB)
          <input type="number" min="0" value={minKb} onChange={(e) => setMinKb(e.target.value)} style={{ width: 80, marginLeft: 6 }} />
        </label>
      </div>
      {folder && <p className="pd-cap">{folder}</p>}
      {busy && <p className="pd-cap">Scanning…</p>}
      {error && <div className="pd-error">{error}</div>}
      {report && (
        <>
          <p>{report.filesScanned.toLocaleString()} files scanned · {report.groups.length} duplicate group(s) · {formatBytes(report.wastedBytes)} could be saved.</p>
          {report.truncated && <div className="pd-warn">The folder has more than 100,000 files, so only part of it was scanned.</div>}
          {report.groups.map((g) => (
            <div key={g.sha256 + g.files[0]} className="pd-panel" style={{ background: "var(--color-surface-soft)" }}>
              <b>{g.files.length} identical files · {formatBytes(g.sizeBytes)} each</b>
              <table className="pd-table"><tbody>
                {g.files.map((f) => (
                  <tr key={f}><td>{f}</td><td style={{ width: 70 }}><button type="button" className="pd-btn secondary small" onClick={() => void invoke("reveal_in_folder", { path: f })}>Show</button></td></tr>
                ))}
              </tbody></table>
            </div>
          ))}
        </>
      )}
    </div>
  );
}

// ---------------- Bulk rename ----------------
type RenameItem = { from: string; to: string; ok: boolean; message: string };
type RenameReport = { items: RenameItem[]; applied: boolean };

function RenameTab() {
  const [paths, setPaths] = useState<string[]>([]);
  const [pattern, setPattern] = useState("{name}");
  const [start, setStart] = useState("1");
  const [pad, setPad] = useState("2");
  const [find, setFind] = useState("");
  const [replace, setReplace] = useState("");
  const [report, setReport] = useState<RenameReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);

  const request = (apply: boolean) => ({
    paths,
    pattern,
    start: Math.max(0, parseInt(start, 10) || 0),
    pad: Math.max(0, parseInt(pad, 10) || 0),
    find,
    replace,
    apply,
  });

  const run = async (apply: boolean) => {
    setError(null);
    try {
      const r = await invoke<RenameReport>("bulk_rename", { request: request(apply) });
      setReport(r);
      setConfirm(false);
      if (apply && r.applied) setPaths(r.items.map((i) => i.to));
    } catch (e) {
      setError(errText(e));
    }
  };

  const choose = async () => {
    const picked = await open({ multiple: true, directory: false });
    if (Array.isArray(picked)) { setPaths(picked); setReport(null); }
  };

  const allOk = report ? report.items.every((i) => i.ok) : false;

  return (
    <div className="pd-panel">
      <p className="pd-cap">Use <b>{"{name}"}</b> for the old name and <b>{"{n}"}</b> for a counter. File extensions are kept. Always preview first.</p>
      <div className="pd-row">
        <button type="button" className="pd-btn" onClick={() => void choose()}>Choose files</button>
        <span className="pd-cap">{paths.length} file(s) selected</span>
      </div>
      <div className="pd-grid2">
        <label className="pd-field">New name pattern<input type="text" value={pattern} onChange={(e) => { setPattern(e.target.value); setConfirm(false); }} /></label>
        <div className="pd-grid2">
          <label className="pd-field">Counter starts at<input type="number" min="0" value={start} onChange={(e) => setStart(e.target.value)} /></label>
          <label className="pd-field">Counter digits<input type="number" min="0" max="12" value={pad} onChange={(e) => setPad(e.target.value)} /></label>
        </div>
        <label className="pd-field">Find in old name (optional)<input type="text" value={find} onChange={(e) => setFind(e.target.value)} /></label>
        <label className="pd-field">Replace with<input type="text" value={replace} onChange={(e) => setReplace(e.target.value)} /></label>
      </div>
      {error && <div className="pd-error">{error}</div>}
      <div className="pd-row">
        <button type="button" className="pd-btn secondary" disabled={paths.length === 0} onClick={() => void run(false)}>Preview</button>
        {!confirm
          ? <button type="button" className="pd-btn" disabled={!report || !allOk || report.applied} onClick={() => setConfirm(true)}>Rename files…</button>
          : <button type="button" className="pd-btn" onClick={() => void run(true)}>Click again to confirm renaming {paths.length} file(s)</button>}
      </div>
      {report && report.applied && <div className="pd-ok">Renamed.</div>}
      {report && (
        <table className="pd-table">
          <thead><tr><th>Old name</th><th>New name</th><th>Status</th></tr></thead>
          <tbody>
            {report.items.map((i) => (
              <tr key={i.from}><td>{baseName(i.from)}</td><td>{baseName(i.to)}</td><td className={i.ok ? "pd-ok-text" : "pd-bad-text"}>{i.ok ? i.message || "OK" : i.message}</td></tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// ---------------- page ----------------
export default function Utilities() {
  const [tab, setTab] = useState<Tab>("hash");
  return (
    <div className="pd-page">
      <h1>Utilities</h1>
      <p className="pd-lead">Everyday file and text tools. All of them run on this PC.</p>
      <div className="pd-tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} type="button" role="tab" aria-selected={tab === t.id} className={`pd-chip ${tab === t.id ? "active" : ""}`} onClick={() => setTab(t.id)}>{t.label}</button>
        ))}
      </div>
      {tab === "hash" && <HashTab />}
      {tab === "qr" && <QrTab />}
      {tab === "base64" && <Base64Tab />}
      {tab === "text" && <TextTab />}
      {tab === "json" && <JsonTab />}
      {tab === "duplicates" && <DuplicatesTab />}
      {tab === "rename" && <RenameTab />}
    </div>
  );
}
