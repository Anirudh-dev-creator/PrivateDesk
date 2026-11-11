import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { errText, formatBytes, numOrNull, stemOf, useFileDrop } from "../lib/helpers";

type Info = { path: string; name: string; bytes: number; width: number; height: number; hasAlpha: boolean; format: string };
type Preview = { dataUrl: string; width: number; height: number; bytes: number; format: string };
type Exif = { tag: string; value: string };
type Saved = { outputPath: string; bytes: number; width: number; height: number };

const EXT: Record<string, string> = { jpeg: "jpg", png: "png", webp: "webp", bmp: "bmp", tiff: "tiff" };

export default function ImageStudio() {
  const [info, setInfo] = useState<Info | null>(null);
  const [before, setBefore] = useState<Preview | null>(null);
  const [after, setAfter] = useState<Preview | null>(null);
  const [exif, setExif] = useState<Exif[] | null>(null);
  const [saved, setSaved] = useState<Saved | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);

  const [rotate, setRotate] = useState(0);
  const [flipH, setFlipH] = useState(false);
  const [flipV, setFlipV] = useState(false);
  const [cropOn, setCropOn] = useState(false);
  const [cx, setCx] = useState("0");
  const [cy, setCy] = useState("0");
  const [cw, setCw] = useState("");
  const [ch, setCh] = useState("");
  const [rw, setRw] = useState("");
  const [rh, setRh] = useState("");
  const [brightness, setBrightness] = useState(0);
  const [contrast, setContrast] = useState(0);
  const [blur, setBlur] = useState(0);
  const [sharpen, setSharpen] = useState(false);
  const [gray, setGray] = useState(false);
  const [sepia, setSepia] = useState(false);
  const [format, setFormat] = useState("jpeg");
  const [quality, setQuality] = useState(90);
  const [dpi, setDpi] = useState("");

  const buildParams = (path: string) => ({
    inputPath: path,
    crop: cropOn
      ? { x: Math.max(0, parseInt(cx, 10) || 0), y: Math.max(0, parseInt(cy, 10) || 0), width: numOrNull(cw) ?? 0, height: numOrNull(ch) ?? 0 }
      : null,
    resizeWidth: numOrNull(rw),
    resizeHeight: numOrNull(rh),
    rotate,
    flipHorizontal: flipH,
    flipVertical: flipV,
    brightness,
    contrast,
    grayscale: gray,
    sepia,
    blur,
    sharpen,
    outputFormat: format,
    jpegQuality: quality,
    dpi: numOrNull(dpi),
  });

  const loadFile = async (path: string) => {
    setError(null);
    setSaved(null);
    setExif(null);
    setBusy(true);
    try {
      const i = await invoke<Info>("image_info", { path });
      setInfo(i);
      setCw(String(i.width));
      setCh(String(i.height));
      const original = await invoke<Preview>("studio_preview", {
        params: { inputPath: path, outputFormat: i.hasAlpha ? "png" : "jpeg", jpegQuality: 90 },
      });
      setBefore(original);
      setAfter(original);
    } catch (e) {
      setInfo(null);
      setBefore(null);
      setAfter(null);
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  useFileDrop((paths) => void loadFile(paths[0]), setDragging);

  const choose = async () => {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"] }] });
    if (typeof picked === "string") await loadFile(picked);
  };

  const updatePreview = async () => {
    if (!info) return;
    setError(null);
    setSaved(null);
    setBusy(true);
    try {
      setAfter(await invoke<Preview>("studio_preview", { params: buildParams(info.path) }));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const saveAs = async () => {
    if (!info) return;
    const folder = info.path.replace(/[\\/][^\\/]*$/, "");
    const ext = EXT[format] ?? "jpg";
    const out = await save({ defaultPath: `${folder}\\${stemOf(info.name)}_edited.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!out) return;
    setError(null);
    setBusy(true);
    try {
      setSaved(await invoke<Saved>("studio_save", { params: buildParams(info.path), outputPath: out }));
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const showExif = async () => {
    if (!info) return;
    try {
      setExif(await invoke<Exif[]>("read_exif", { path: info.path }));
    } catch (e) {
      setError(errText(e));
    }
  };

  return (
    <div className="pd-page">
      <h1>Image Studio</h1>
      <p className="pd-lead">Resize, crop, rotate and adjust photos. Saved images contain no EXIF/location data. Originals are never changed.</p>

      <div className={`pd-drop ${dragging ? "drag" : ""}`} onClick={() => !busy && void choose()} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter") void choose(); }}>
        <b>{info ? info.name : "Drop an image here or click to choose"}</b>
        <small>{info ? `${info.width} × ${info.height}px · ${info.format} · ${formatBytes(info.bytes)}` : "JPG, PNG, WebP, BMP, TIFF · up to 40 megapixels"}</small>
      </div>

      {error && <div className="pd-error" role="alert">{error}</div>}
      {saved && <div className="pd-ok">Saved and verified on disk: {saved.outputPath} ({formatBytes(saved.bytes)}, {saved.width} × {saved.height}px)</div>}

      {info && (
        <div className="pd-split">
          <div className="pd-panel">
            <h2>Edits</h2>
            <div className="pd-grid2">
              <label className="pd-field">Rotate
                <select value={rotate} onChange={(e) => setRotate(Number(e.target.value))}>
                  <option value={0}>None</option><option value={90}>90° clockwise</option><option value={180}>180°</option><option value={270}>90° counter-clockwise</option>
                </select>
              </label>
              <div>
                <label className="pd-check"><input type="checkbox" checked={flipH} onChange={(e) => setFlipH(e.target.checked)} /> Flip horizontal</label>
                <label className="pd-check"><input type="checkbox" checked={flipV} onChange={(e) => setFlipV(e.target.checked)} /> Flip vertical</label>
              </div>
            </div>

            <label className="pd-check"><input type="checkbox" checked={cropOn} onChange={(e) => setCropOn(e.target.checked)} /> Crop (pixels, from top-left)</label>
            {cropOn && (
              <div className="pd-grid2">
                <label className="pd-field">X<input type="number" min="0" value={cx} onChange={(e) => setCx(e.target.value)} /></label>
                <label className="pd-field">Y<input type="number" min="0" value={cy} onChange={(e) => setCy(e.target.value)} /></label>
                <label className="pd-field">Width<input type="number" min="1" value={cw} onChange={(e) => setCw(e.target.value)} /></label>
                <label className="pd-field">Height<input type="number" min="1" value={ch} onChange={(e) => setCh(e.target.value)} /></label>
              </div>
            )}

            <div className="pd-grid2">
              <label className="pd-field">Resize width (px)<input type="number" min="1" placeholder="keep" value={rw} onChange={(e) => setRw(e.target.value)} /></label>
              <label className="pd-field">Resize height (px)<input type="number" min="1" placeholder="keep" value={rh} onChange={(e) => setRh(e.target.value)} /></label>
            </div>
            <p className="pd-cap" style={{ marginTop: -4 }}>Fill only one to keep the proportions.</p>

            <label className="pd-field">Brightness: {brightness}<input type="range" min={-100} max={100} value={brightness} onChange={(e) => setBrightness(Number(e.target.value))} /></label>
            <label className="pd-field">Contrast: {contrast}<input type="range" min={-50} max={50} value={contrast} onChange={(e) => setContrast(Number(e.target.value))} /></label>
            <label className="pd-field">Blur: {blur}<input type="range" min={0} max={10} step={0.5} value={blur} onChange={(e) => setBlur(Number(e.target.value))} /></label>
            <label className="pd-check"><input type="checkbox" checked={sharpen} onChange={(e) => setSharpen(e.target.checked)} /> Sharpen</label>
            <label className="pd-check"><input type="checkbox" checked={gray} onChange={(e) => setGray(e.target.checked)} /> Grayscale</label>
            <label className="pd-check"><input type="checkbox" checked={sepia} onChange={(e) => setSepia(e.target.checked)} /> Sepia</label>

            <h2 style={{ marginTop: 14 }}>Output</h2>
            <label className="pd-field">Format
              <select value={format} onChange={(e) => setFormat(e.target.value)}>
                <option value="jpeg">JPEG (transparency becomes white)</option><option value="png">PNG</option>
                <option value="webp">WebP (lossless)</option><option value="bmp">BMP</option><option value="tiff">TIFF</option>
              </select>
            </label>
            {format === "jpeg" && (
              <>
                <label className="pd-field">JPEG quality: {quality}<input type="range" min={10} max={100} value={quality} onChange={(e) => setQuality(Number(e.target.value))} /></label>
                <label className="pd-field">DPI tag (optional)<input type="number" min="1" max="1200" placeholder="e.g. 300" value={dpi} onChange={(e) => setDpi(e.target.value)} /></label>
              </>
            )}
            <div className="pd-row">
              <button type="button" className="pd-btn secondary" disabled={busy} onClick={() => void updatePreview()}>Update preview</button>
              <button type="button" className="pd-btn" disabled={busy} onClick={() => void saveAs()}>Save as…</button>
            </div>
            <button type="button" className="pd-btn secondary small" onClick={() => void showExif()}>View EXIF data of original</button>
          </div>

          <div>
            <div className="pd-panel">
              <div className="pd-previews">
                <div>
                  <div className="pd-preview">{before && <img src={before.dataUrl} alt="Original" />}</div>
                  <p className="pd-cap">Original · {info.width} × {info.height}px</p>
                </div>
                <div>
                  <div className="pd-preview">{after && <img src={after.dataUrl} alt="Edited preview" />}</div>
                  <p className="pd-cap">{after ? `Edited · ${after.width} × ${after.height}px · about ${formatBytes(after.bytes)} as ${after.format.toUpperCase()}` : ""}</p>
                </div>
              </div>
              <p className="pd-cap">Previews are scaled down for display. Saved files use the full resolution.</p>
            </div>
            {exif && (
              <div className="pd-panel">
                <h2>EXIF data</h2>
                {exif.length === 0 ? <p className="pd-cap">No EXIF metadata found in this file.</p> : (
                  <table className="pd-table"><tbody>{exif.map((x, i) => <tr key={i}><td>{x.tag}</td><td>{x.value}</td></tr>)}</tbody></table>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
