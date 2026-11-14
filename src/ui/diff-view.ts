// Virtualized diff list: only the rows in view exist in the DOM. One vertical
// scroller holds both columns, so they always move together; horizontal
// scrolling is a shared offset applied to the text of every row.

import { plural } from "../i18n";
import { icon } from "../icons";
import type { DiffData, Span } from "../ipc";
import { blockEnd, buildRows, CONTEXT, DELETE, FOLD, INSERT, type Row } from "./rows";

const ROW_HEIGHT = 20;
const OVERSCAN = 12;
const TAB_SIZE = 4;

const kindClass = ["k-eq", "k-del", "k-ins", "k-mod", "k-fold"];

export class DiffView {
  private readonly scroller: HTMLElement;
  private readonly canvas: HTMLElement;
  private readonly bars: HTMLElement;

  private data: DiffData | null = null;
  private unified = false;
  private onlyChanges = false;
  private expanded = new Set<number>();
  private rows: Row[] = [];
  private changes: number[] = [];

  /** Rendered rows by row index. */
  private live = new Map<number, HTMLElement>();
  private frame = 0;

  private charWidth = 7.2;
  private textWidth = 0;
  private offsetX = 0;
  private maxOffsetX = 0;

  /** Change block reached with next/previous, or -1 after a manual scroll. */
  private current = -1;
  private navigatingUntil = 0;

  constructor(scroller: HTMLElement, canvas: HTMLElement, bars: HTMLElement) {
    this.scroller = scroller;
    this.canvas = canvas;
    this.bars = bars;

    scroller.addEventListener("scroll", () => {
      if (performance.now() > this.navigatingUntil) this.current = -1;
      this.schedule();
    });
    scroller.addEventListener("wheel", (e) => this.onWheel(e), { passive: false });
    new ResizeObserver(() => {
      this.layoutBars();
      this.schedule();
    }).observe(scroller);
    canvas.addEventListener("click", (e) => {
      const button = (e.target as Element).closest<HTMLElement>(".fold-button");
      if (button) this.expand(Number(button.dataset.op));
    });
  }

  get changeCount(): number {
    return this.changes.length;
  }

  get rowCount(): number {
    return this.rows.length;
  }

  /**
   * New data or new view options. With `keepPosition` (same files, or only
   * the options changed) the first visible line stays in place; otherwise the
   * view starts from the top.
   */
  show(data: DiffData, unified: boolean, onlyChanges: boolean, keepPosition: boolean): void {
    const anchor = keepPosition ? this.topLine() : null;
    if (data !== this.data) {
      if (!keepPosition) this.expanded.clear();
      this.measureText(data);
    }
    this.data = data;
    this.unified = unified;
    this.onlyChanges = onlyChanges;
    this.rebuild();
    if (anchor) {
      this.scrollToLine(anchor);
    } else {
      this.scroller.scrollTop = 0;
      this.setOffsetX(0);
    }
  }

  next(): void {
    this.go(1);
  }

  prev(): void {
    this.go(-1);
  }

  private rebuild(): void {
    if (!this.data) return;
    const model = buildRows(this.data, this.unified, this.onlyChanges, this.expanded);
    this.rows = model.rows;
    this.changes = model.changes;
    this.current = -1;
    this.canvas.classList.toggle("unified", this.unified);
    this.canvas.style.height = `${this.rows.length * ROW_HEIGHT}px`;
    this.clearRows();
    this.layoutBars();
    this.render();
  }

  private expand(op: number): void {
    this.expanded.add(op);
    const top = this.scroller.scrollTop;
    this.rebuild();
    this.scroller.scrollTop = top;
  }

  private clearRows(): void {
    for (const el of this.live.values()) el.remove();
    this.live.clear();
  }

  private schedule(): void {
    if (!this.frame) {
      this.frame = requestAnimationFrame(() => {
        this.frame = 0;
        this.render();
      });
    }
  }

  private render(): void {
    if (!this.data) return;
    const top = this.scroller.scrollTop;
    const height = this.scroller.clientHeight;
    const first = Math.max(0, Math.floor(top / ROW_HEIGHT) - OVERSCAN);
    const last = Math.min(this.rows.length, Math.ceil((top + height) / ROW_HEIGHT) + OVERSCAN);

    for (const [index, el] of this.live) {
      if (index < first || index >= last) {
        el.remove();
        this.live.delete(index);
      }
    }
    const currentStart = this.current >= 0 ? this.changes[this.current] : -1;
    const currentEnd = currentStart >= 0 ? blockEnd(this.rows, currentStart) : -1;
    for (let index = first; index < last; index++) {
      let el = this.live.get(index);
      if (!el) {
        el = this.renderRow(this.rows[index]);
        el.style.transform = `translateY(${index * ROW_HEIGHT}px)`;
        this.canvas.append(el);
        this.live.set(index, el);
      }
      el.classList.toggle("current", index >= currentStart && index < currentEnd);
    }
  }

  private renderRow(row: Row): HTMLElement {
    const el = document.createElement("div");
    el.className = `row ${kindClass[row.kind]}`;
    if (row.kind === FOLD) {
      const button = document.createElement("button");
      button.className = "fold-button";
      button.dataset.op = String(row.op);
      button.append(icon("ChevronsUpDown", 12), plural("fold.show", row.hidden ?? 0));
      el.append(button);
      return el;
    }
    const data = this.data!;
    const oldText = row.a >= 0 ? data.oldLines[row.a] : "";
    const newText = row.b >= 0 ? data.newLines[row.b] : "";
    if (this.unified) {
      const isNew = row.kind === INSERT;
      el.append(
        number(row.a),
        number(row.b),
        cell("sign", row.kind === DELETE ? "−" : isNew ? "+" : ""),
        code(isNew ? newText : oldText, isNew ? row.wordsB : row.wordsA, isNew ? "wi" : "wd"),
      );
    } else {
      el.append(
        number(row.a, "side-a"),
        code(oldText, row.wordsA, "wd", "side-a"),
        number(row.b, "side-b"),
        code(newText, row.wordsB, "wi", "side-b"),
      );
    }
    return el;
  }

  /** First visible line, as a line of A or B, to restore after a rebuild. */
  private topLine(): { a: number; b: number } | null {
    const start = Math.floor(this.scroller.scrollTop / ROW_HEIGHT);
    for (let i = start; i < this.rows.length; i++) {
      const { a, b } = this.rows[i];
      if (a >= 0 || b >= 0) return { a, b };
    }
    return null;
  }

  private scrollToLine(line: { a: number; b: number }): void {
    const index = this.rows.findIndex((r) => (line.a >= 0 ? r.a >= line.a : r.b >= line.b));
    if (index >= 0) this.scroller.scrollTop = index * ROW_HEIGHT;
  }

  private go(step: 1 | -1): void {
    if (this.changes.length === 0) return;
    let target: number;
    if (this.current >= 0) {
      target = Math.min(Math.max(this.current + step, 0), this.changes.length - 1);
    } else {
      // From wherever the user scrolled to: the reading line sits CONTEXT rows below the top.
      const reading = Math.floor(this.scroller.scrollTop / ROW_HEIGHT) + CONTEXT;
      if (step === 1) {
        target = this.changes.findIndex((row) => row > reading);
        if (target < 0) target = this.changes.length - 1;
      } else {
        target = 0;
        for (let i = this.changes.length - 1; i > 0; i--) {
          if (this.changes[i] < reading) {
            target = i;
            break;
          }
        }
      }
    }
    this.current = target;
    this.navigatingUntil = performance.now() + 150;
    this.scroller.scrollTop = Math.max(0, (this.changes[target] - CONTEXT) * ROW_HEIGHT);
    this.render();
  }

  // Horizontal scrolling

  private measureText(data: DiffData): void {
    const probe = document.createElement("span");
    probe.className = "measure";
    probe.textContent = "0".repeat(100);
    this.canvas.append(probe);
    this.charWidth = probe.getBoundingClientRect().width / 100 || this.charWidth;
    probe.remove();

    let longest = 0;
    for (const lines of [data.oldLines, data.newLines]) {
      for (const line of lines) {
        // A tab is at most TAB_SIZE columns wide, so short lines can be skipped.
        if (line.length * TAB_SIZE > longest) longest = Math.max(longest, visualLength(line));
      }
    }
    this.textWidth = Math.ceil(longest * this.charWidth) + 24;

    const digits = String(Math.max(data.oldLines.length, data.newLines.length, 1)).length;
    this.canvas.style.setProperty("--gutter", `${Math.ceil(digits * this.charWidth) + 20}px`);
  }

  private onWheel(e: WheelEvent): void {
    let dx = e.deltaX;
    if (dx === 0 && e.shiftKey) dx = e.deltaY;
    if (dx === 0 || Math.abs(dx) < Math.abs(e.deltaY) && !e.shiftKey) return;
    if (this.maxOffsetX === 0) return;
    e.preventDefault();
    this.setOffsetX(this.offsetX + dx);
  }

  private setOffsetX(x: number, from?: HTMLElement): void {
    this.offsetX = Math.min(Math.max(0, x), this.maxOffsetX);
    this.canvas.style.setProperty("--sx", `${this.offsetX}px`);
    for (const bar of this.bars.children) {
      if (bar !== from) (bar as HTMLElement).scrollLeft = this.offsetX;
    }
  }

  /** One native horizontal scrollbar under each text column. */
  private layoutBars(): void {
    const gutter = parseFloat(getComputedStyle(this.canvas).getPropertyValue("--gutter")) || 48;
    const width = this.scroller.clientWidth;
    const columns: [number, number][] = this.unified
      ? [[gutter * 2 + 18, width]]
      : [
          [gutter, width / 2],
          [width / 2 + gutter, width],
        ];
    const visible = Math.min(...columns.map(([left, right]) => right - left));
    this.maxOffsetX = Math.max(0, this.textWidth - visible);

    this.bars.replaceChildren();
    if (this.maxOffsetX > 0) {
      for (const [left, right] of columns) {
        const bar = document.createElement("div");
        bar.className = "hbar";
        bar.style.left = `${left}px`;
        bar.style.width = `${right - left}px`;
        const inner = document.createElement("div");
        inner.style.width = `${this.textWidth}px`;
        bar.append(inner);
        bar.addEventListener("scroll", () => {
          if (bar.scrollLeft !== this.offsetX) this.setOffsetX(bar.scrollLeft, bar);
        });
        this.bars.append(bar);
      }
    }
    this.setOffsetX(this.offsetX);
  }
}

function number(line: number, side?: string): HTMLElement {
  const el = cell("num", line >= 0 ? String(line + 1) : "");
  if (side) el.classList.add(side);
  return el;
}

function cell(className: string, text: string): HTMLElement {
  const el = document.createElement("span");
  el.className = className;
  el.textContent = text;
  return el;
}

function code(text: string, words: Span[] | undefined, wordClass: string, side?: string): HTMLElement {
  const el = document.createElement("span");
  el.className = side ? `code ${side}` : "code";
  const inner = document.createElement("span");
  inner.className = "text";
  if (!words || words.length === 0) {
    inner.textContent = text;
  } else {
    let pos = 0;
    for (const [start, end] of words) {
      if (start > pos) inner.append(text.slice(pos, start));
      inner.append(cell(wordClass, text.slice(start, end)));
      pos = end;
    }
    if (pos < text.length) inner.append(text.slice(pos));
  }
  el.append(inner);
  return el;
}

/** Length in columns, with tabs expanded. */
function visualLength(line: string): number {
  let length = 0;
  for (let i = 0; i < line.length; i++) {
    length += line.charCodeAt(i) === 9 ? TAB_SIZE - (length % TAB_SIZE) : 1;
  }
  return length;
}

