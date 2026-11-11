import { useEffect, useRef } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Decimal units: 1 KB = 1,000 bytes. */
export function formatBytes(bytes: number): string {
  if (bytes < 1000) return `${bytes.toLocaleString()} bytes`;
  if (bytes < 1_000_000) return `${(bytes / 1000).toFixed(1)} KB (${bytes.toLocaleString()} bytes)`;
  return `${(bytes / 1_000_000).toFixed(2)} MB (${bytes.toLocaleString()} bytes)`;
}

export function stemOf(name: string): string {
  return name.replace(/\.[^.]+$/, "");
}

export function folderOf(path: string): string {
  return path.replace(/[\\/][^\\/]*$/, "");
}

export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function numOrNull(text: string): number | null {
  const n = parseInt(text, 10);
  return Number.isFinite(n) && n > 0 ? n : null;
}

/** Calls onPaths when files are dropped on the window. */
export function useFileDrop(onPaths: (paths: string[]) => void, onDragging?: (dragging: boolean) => void) {
  const pathsRef = useRef(onPaths);
  const dragRef = useRef(onDragging);
  pathsRef.current = onPaths;
  dragRef.current = onDragging;
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") dragRef.current?.(true);
        if (event.payload.type === "leave") dragRef.current?.(false);
        if (event.payload.type === "drop") {
          dragRef.current?.(false);
          const paths = event.payload.paths;
          if (paths && paths.length > 0) pathsRef.current(paths);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
