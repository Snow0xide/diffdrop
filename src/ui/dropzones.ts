// Zones A and B. Before a comparison they fill the window and invite a drop;
// during one they shrink to a header above each column. Drops come from
// Tauri, which reports the cursor position, so the zone is chosen by position.

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { t } from "../i18n";
import { icon, type IconName } from "../icons";
import { api, type FileError, type FileView, type Side, type Snapshot } from "../ipc";

const SIDES: Side[] = ["a", "b"];

interface Drag {
  paths: string[];
  side: Side;
}

export class DropZones {
  private readonly root: HTMLElement;
  private readonly zones: HTMLElement[];
  private readonly overlay: HTMLElement;
  /** Content currently drawn in each zone, to avoid restarting animations. */
  private drawn = ["", ""];
  private drag: Drag | null = null;
  private paths: string[] = [];
  private compact = false;

  constructor(root: HTMLElement, overlay: HTMLElement) {
    this.root = root;
    this.overlay = overlay;
    this.zones = SIDES.map((side) => root.querySelector<HTMLElement>(`.zone[data-side="${side}"]`)!);
    this.zones.forEach((zone, i) => zone.addEventListener("click", () => api.choose(SIDES[i])));

    getCurrentWebview().onDragDropEvent(({ payload }) => {
      switch (payload.type) {
        case "enter":
          this.paths = payload.paths;
          this.setDrag(this.sideAt(payload.position.x));
          break;
        case "over":
          this.setDrag(this.sideAt(payload.position.x));
          break;
        case "drop": {
          const side = this.sideAt(payload.position.x);
          this.setDrag(null);
          if (payload.paths.length > 0) {
            // Two or more files fill A and B in order, wherever they land.
            api.openPaths(payload.paths, payload.paths.length > 1 ? null : side);
          }
          break;
        }
        case "leave":
          this.setDrag(null);
          break;
      }
    });
  }

  render(session: Snapshot, compact: boolean): void {
    this.compact = compact;
    this.root.classList.toggle("compact", compact);
    SIDES.forEach((side, i) => this.renderZone(i, side, session.files[i], session.errors[i]));
  }

  /** Positions arrive in physical pixels relative to the webview. */
  private sideAt(physicalX: number): Side {
    const x = physicalX / window.devicePixelRatio;
    const a = this.zones[0].getBoundingClientRect();
    const b = this.zones[1].getBoundingClientRect();
    return x < (a.right + b.left) / 2 ? "a" : "b";
  }

  private setDrag(side: Side | null): void {
    const drag = side ? { paths: this.paths, side } : null;
    if ((drag?.side ?? null) === (this.drag?.side ?? null)) return;
    this.drag = drag;

    const both = (drag?.paths.length ?? 0) > 1;
    this.zones.forEach((zone, i) => {
      zone.toggleAttribute("data-hover", drag !== null && (both || drag.side === SIDES[i]));
    });
    this.root.dataset.dropHint = drag ? (both ? t("zone.dropBoth") : "") : "";

    this.overlay.hidden = !(drag && this.compact);
    if (drag && this.compact) {
      SIDES.forEach((side, i) => {
        const half = this.overlay.children[i] as HTMLElement;
        const target = both || drag.side === side;
        half.classList.toggle("active", target);
        const path = both ? drag.paths[i] : target ? drag.paths[0] : undefined;
        half.replaceChildren(
          badge(side),
          text("drop-text", target ? t("zone.dropHere", { side: side.toUpperCase() }) : ""),
          text("drop-file", path ? basename(path) : ""),
        );
      });
    }
    // The zone hint reflects the drag in the full-size layout.
    this.zones.forEach((zone, i) => {
      const hint = zone.querySelector<HTMLElement>(".zone-drop");
      if (hint) {
        hint.textContent =
          drag && !both && drag.side === SIDES[i] ? t("zone.dropHere", { side: SIDES[i].toUpperCase() }) : "";
      }
    });
  }

  private renderZone(i: number, side: Side, file: FileView | null, error: FileError | null): void {
    const key = JSON.stringify([file?.path, error, this.compact]);
    if (this.drawn[i] === key) return;
    this.drawn[i] = key;

    const zone = this.zones[i];
    zone.dataset.state = error ? "error" : file ? "filled" : "empty";
    zone.title = file ? `${file.path}\n${t("zone.choose")}` : "";
    const iconName: IconName = error ? "TriangleAlert" : file ? "FileText" : "FileUp";

    const parts: Node[] = [
      box("zone-icon", icon(iconName, compactIconSize(this.compact))),
      box("zone-label", badge(side), text("zone-title", t(side === "a" ? "zone.a" : "zone.b"))),
    ];
    if (file) {
      parts.push(text("zone-name", file.name), text("zone-path", parent(file.path)));
    } else {
      parts.push(text("zone-hint", t("zone.hint")), text("zone-sub", t("zone.hintClick")));
    }
    if (error) parts.push(text("zone-error", errorMessage(error)));
    parts.push(text("zone-drop", ""));
    zone.replaceChildren(...parts);
  }
}

function compactIconSize(compact: boolean): number {
  return compact ? 14 : 28;
}

function errorMessage(error: FileError): string {
  switch (error.kind) {
    case "binary":
      return t("error.binary");
    case "folder":
      return t("error.folder");
    case "unreadable":
      return t("error.unreadable", { reason: error.reason });
  }
}

function badge(side: Side): HTMLElement {
  return text("badge", side.toUpperCase());
}

function text(className: string, value: string): HTMLElement {
  const el = document.createElement("span");
  el.className = className;
  el.textContent = value;
  return el;
}

function box(className: string, ...children: Node[]): HTMLElement {
  const el = document.createElement("span");
  el.className = className;
  el.append(...children);
  return el;
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function parent(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : "";
}
