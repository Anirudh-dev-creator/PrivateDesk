import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { errText, formatBytes, stemOf, useFileDrop, baseName } from "../lib/helpers";

type Preview = { dataUrl: string; width: number; height: number; count: number };
type Saved = { outputPath: string; bytes: number; width: number; height: number };

// Sizes are common defaults only. Always check the official rules of the office or portal you are applying to.
const PRESETS = [
  { label: "India passport (35 × 45 mm)", w: 35, h: 45 },
  { label: "India visa / US passport (51 × 51 mm, 2 × 2 in)", w: 51, h: 51 },
  { label: "PAN card (25 × 35 mm)", w: 25, h: 35 },
  { label: "UK passport (35 × 45 mm)", w: 35, h: 45 },
  { label: "Schengen visa (35 × 45 mm)", w: 35, h: 45 },
  { label: "Stamp size (20 × 25 mm)", w: 20, h: 25 },
  { label: "Custom", w: 35, h: 45 },
];
const PAPERS = [
  { label: "4 × 6 in", w: 101.6, h: 152.4 },
  { label: "5 × 7 in", w: 127, h: 177.8 },
  { label: "A4", w: 210, h: 297 },
  { label: "Custom", w: 210, h: 297 },
];
const BACKGROUNDS = [
  { label: "White", color: "#ffffff" },
  { label: "Light blue", color: "#d6e6f5" },
  { label: "Light grey", color: "#e6e6e6" },
];

export default function PassportStudio() {
  const [path, setPath] = useState<string | null>(null);
  const [preset, setPreset] = useState(0);
  const [wMm, setWMm] = useState("35");
  const [hMm, setHMm] = useState("45");
  const [dpi, setDpi] = useState("300");
  const [zoom, setZoom] = useState(1);
  const [ox, setOx] = useState(0);
  const [oy, setOy] = useState(0);
  const [bg, setBg] = useState("#ffffff");
  const [brightness, setBrightness] = useState(0);
  const [contrast, setContrast] = useState(0);
  const [view, setView] = useState<"photo" | "sheet">("photo");
  const [paper, setPaper] = useState(0);
  const [pwMm, setPwMm] = useState("101.6");
  const [phMm, setPhMm] = useState("152.4");
  const [margin, setMargin] = useState("3");
  const [spacing, setSpacing] = useState("2");
  const [count, setCount] = useState("0");
  const [guides, setGuides] = useState(true);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [saved, setSaved] = useState<Saved | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const requestId = useRef(0);

  const photoParams = () => ({
    inputPath: path ?? "",
    widthMm: Number(wMm),
    heightMm: Number(hMm),
    dpi: parseInt(dpi, 10) || 300,
    zoom,
    offsetX: ox,
    offsetY: oy,
    background: bg,
    brightness,
    contrast,
  });
  const sheetParams = () => ({
    photo: photoParams(),
    paperWidthMm: Number(pwMm),
    paperHeightMm: Number(phMm),
    marginMm: Number(margin) || 0,
    spacingMm: Number(spacing) || 0,
    count: Math.max(0, parseInt(count, 10) || 0),
    cutGuides: guides,
  });

  useFileDrop((paths) => { setPath(paths[0]); setSaved(null); }, setDragging);

  // Live preview, slightly delayed so sliders stay smooth.
  useEffect(() => {
    if (!path) return;
    const id = ++requestId.current;
    const timer = window.setTimeout(async () => {
      try {
        const result = view === "photo"
          ? await invoke<Preview>("passport_preview", { params: photoParams() })
          : await invoke<Preview>("sheet_preview", { params: sheetParams() });
        if (id === requestId.current) {
          setPreview(result);
          setError(null);
        }
      } catch (e) {
        if (id === requestId.current) setError(errText(e));
      }
    }, 350);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, wMm, hMm, dpi, zoom, ox, oy, bg, brightness, contrast, view, pwMm, phMm, margin, spacing, count, guides]);

  const choose = async () => {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"] }] });
    if (typeof picked === "string") { setPath(picked); setSaved(null); }
  };

  const pickPreset = (index: number) => {
    setPreset(index);
    if (PRESETS[index].label !== "Custom") {
      setWMm(String(PRESETS[index].w));
      setHMm(String(PRESETS[index].h));
    }
  };
  const pickPaper = (index: number) => {
    setPaper(index);
    if (PAPERS[index].label !== "Custom") {
      setPwMm(String(PAPERS[index].w));
      setPhMm(String(PAPERS[index].h));
    }
  };

  const exportFile = async (kind: "photo" | "sheet", format: "jpeg" | "png") => {
    if (!path) return;
    const folder = path.replace(/[\\/][^\\/]*$/, "");
    const ext = format === "jpeg" ? "jpg" : "png";
    const out = await save({
      defaultPath: `${folder}\\${stemOf(baseName(path))}_${kind === "photo" ? "passport" : "sheet"}.${ext}`,
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
    });
    if (!out) return;
    setBusy(true);
    setError(null);
    try {
      setSaved(kind === "photo"
        ? await invoke<Saved>("passport_export", { params: photoParams(), outputPath: out, format })
        : await invoke<Saved>("sheet_export", { params: sheetParams(), outputPath: out, format }));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pd-page">
      <h1>Passport & ID Photo Studio</h1>
      <p className="pd-lead">Crop a photo to an exact size and print several copies on one sheet.</p>
      <div className="pd-warn">
        Position the face yourself with the zoom and move sliders. This version has <b>no automatic face detection or background removal</b> (use a photo taken against a plain background).
        Sizes below are common defaults; <b>check the official rules</b> of the passport office or portal. Automatic cropping cannot guarantee acceptance.
      </div>

      <div className={`pd-drop ${dragging ? "drag" : ""}`} onClick={() => void choose()} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter") void choose(); }}>
        <b>{path ? baseName(path) : "Drop a portrait photo here or click to choose"}</b>
        <small>JPG, PNG, WebP, BMP, TIFF</small>
      </div>

      {error && <div className="pd-error" role="alert">{error}</div>}
      {saved && <div className="pd-ok">Saved and verified on disk: {saved.outputPath} ({formatBytes(saved.bytes)}, {saved.width} × {saved.height}px)</div>}

      <div className="pd-split">
        <div>
          <div className="pd-panel">
            <h2>Photo</h2>
            <label className="pd-field">Size preset
              <select value={preset} onChange={(e) => pickPreset(Number(e.target.value))}>
                {PRESETS.map((p, i) => <option key={p.label} value={i}>{p.label}</option>)}
              </select>
            </label>
            <div className="pd-grid2">
              <label className="pd-field">Width (mm)<input type="number" min="10" step="0.1" value={wMm} onChange={(e) => { setWMm(e.target.value); setPreset(PRESETS.length - 1); }} /></label>
              <label className="pd-field">Height (mm)<input type="number" min="10" step="0.1" value={hMm} onChange={(e) => { setHMm(e.target.value); setPreset(PRESETS.length - 1); }} /></label>
            </div>
            <label className="pd-field">Resolution (DPI)<input type="number" min="100" max="600" value={dpi} onChange={(e) => setDpi(e.target.value)} /></label>
            <label className="pd-field">Zoom: {zoom.toFixed(2)}×<input type="range" min={1} max={3} step={0.05} value={zoom} onChange={(e) => setZoom(Number(e.target.value))} /></label>
            <label className="pd-field">Move left / right<input type="range" min={-1} max={1} step={0.02} value={ox} onChange={(e) => setOx(Number(e.target.value))} /></label>
            <label className="pd-field">Move up / down<input type="range" min={-1} max={1} step={0.02} value={oy} onChange={(e) => setOy(Number(e.target.value))} /></label>
            <label className="pd-field">Brightness: {brightness}<input type="range" min={-50} max={50} value={brightness} onChange={(e) => setBrightness(Number(e.target.value))} /></label>
            <label className="pd-field">Contrast: {contrast}<input type="range" min={-40} max={40} value={contrast} onChange={(e) => setContrast(Number(e.target.value))} /></label>
            <div className="pd-row">
              {BACKGROUNDS.map((b) => (
                <button key={b.color} type="button" className={`pd-chip ${bg === b.color ? "active" : ""}`} onClick={() => setBg(b.color)}>{b.label}</button>
              ))}
              <input type="color" value={bg} onChange={(e) => setBg(e.target.value)} aria-label="Custom background colour" />
            </div>
            <p className="pd-cap">Background colour only shows through transparent areas of the photo.</p>
            <div className="pd-row">
              <button type="button" className="pd-btn" disabled={!path || busy} onClick={() => void exportFile("photo", "jpeg")}>Save photo (JPG)</button>
              <button type="button" className="pd-btn secondary" disabled={!path || busy} onClick={() => void exportFile("photo", "png")}>PNG</button>
            </div>
          </div>

          <div className="pd-panel">
            <h2>Print sheet</h2>
            <label className="pd-field">Paper
              <select value={paper} onChange={(e) => pickPaper(Number(e.target.value))}>
                {PAPERS.map((p, i) => <option key={p.label} value={i}>{p.label}</option>)}
              </select>
            </label>
            <div className="pd-grid2">
              <label className="pd-field">Paper width (mm)<input type="number" min="20" step="0.1" value={pwMm} onChange={(e) => { setPwMm(e.target.value); setPaper(PAPERS.length - 1); }} /></label>
              <label className="pd-field">Paper height (mm)<input type="number" min="20" step="0.1" value={phMm} onChange={(e) => { setPhMm(e.target.value); setPaper(PAPERS.length - 1); }} /></label>
              <label className="pd-field">Margin (mm)<input type="number" min="0" step="0.5" value={margin} onChange={(e) => setMargin(e.target.value)} /></label>
              <label className="pd-field">Spacing (mm)<input type="number" min="0" step="0.5" value={spacing} onChange={(e) => setSpacing(e.target.value)} /></label>
            </div>
            <label className="pd-field">Number of photos (0 = as many as fit)<input type="number" min="0" value={count} onChange={(e) => setCount(e.target.value)} /></label>
            <label className="pd-check"><input type="checkbox" checked={guides} onChange={(e) => setGuides(e.target.checked)} /> Cutting guides (needs spacing of 1 mm or more)</label>
            <div className="pd-row">
              <button type="button" className="pd-btn" disabled={!path || busy} onClick={() => void exportFile("sheet", "jpeg")}>Save sheet (JPG)</button>
              <button type="button" className="pd-btn secondary" disabled={!path || busy} onClick={() => void exportFile("sheet", "png")}>PNG</button>
            </div>
            <p className="pd-cap">To print at actual size, open the saved JPG in Windows Photos or Paint and choose “Full page photo” or 100% scale.</p>
          </div>
        </div>

        <div className="pd-panel">
          <div className="pd-row">
            <button type="button" className={`pd-chip ${view === "photo" ? "active" : ""}`} onClick={() => setView("photo")}>Single photo</button>
            <button type="button" className={`pd-chip ${view === "sheet" ? "active" : ""}`} onClick={() => setView("sheet")}>Print sheet</button>
          </div>
          <div className="pd-previews single">
            <div className="pd-preview" style={{ minHeight: 300 }}>
              {preview && path ? <img src={preview.dataUrl} alt="Preview" /> : <span className="pd-cap">Choose a photo to see the preview.</span>}
            </div>
          </div>
          {preview && path && (
            <p className="pd-cap">
              {view === "photo"
                ? `Output: ${preview.width} × ${preview.height}px`
                : `Sheet: ${preview.width} × ${preview.height}px · ${preview.count} photo${preview.count === 1 ? "" : "s"}`}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
