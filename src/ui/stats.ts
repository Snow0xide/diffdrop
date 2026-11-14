// Status bar: added and removed lines, similarity, line ending warning.

import { formatNumber, plural, t, type Key } from "../i18n";
import { icon } from "../icons";
import type { EolStyle } from "../ipc";
import type { State } from "../state";

/** Show "Comparing…" only when it takes long enough to notice. */
const BUSY_DELAY = 250;

export class StatusBar {
  private readonly root: HTMLElement;
  private busyTimer = 0;

  constructor(root: HTMLElement) {
    this.root = root;
  }

  render(state: State, comparing: boolean): void {
    clearTimeout(this.busyTimer);
    const busy = state.session.status === "computing";
    if (busy) {
      this.busyTimer = window.setTimeout(() => this.showBusy(), BUSY_DELAY);
    }
    this.root.hidden = !comparing;
    if (!comparing || !state.diff) {
      this.root.replaceChildren();
      return;
    }

    const { stats } = state.diff;
    const parts: Node[] = [];
    if (stats.added === 0 && stats.removed === 0) {
      parts.push(item("same", t("stats.same")));
    } else {
      parts.push(item("added", plural("stats.added", stats.added)));
      parts.push(item("removed", plural("stats.removed", stats.removed)));
    }
    parts.push(item("similarity", t("stats.similarity", { p: formatNumber(Math.floor(stats.similarity * 100)) })));
    if (stats.eolDiffers) {
      const warning = item("warning", t("stats.eol", { a: eolName(stats.oldEol), b: eolName(stats.newEol) }));
      warning.prepend(icon("TriangleAlert", 12));
      parts.push(warning);
    }
    this.root.replaceChildren(...parts);
  }

  private showBusy(): void {
    const busy = item("busy", t("stats.computing"));
    busy.prepend(icon("LoaderCircle", 12));
    this.root.hidden = false;
    this.root.querySelector(".busy")?.remove();
    this.root.append(busy);
  }
}

function eolName(style: EolStyle): string {
  return t(`stats.eol.${style === "none" ? "lf" : style}` as Key);
}

function item(className: string, text: string): HTMLElement {
  const el = document.createElement("span");
  el.className = `stat ${className}`;
  el.textContent = text;
  return el;
}
