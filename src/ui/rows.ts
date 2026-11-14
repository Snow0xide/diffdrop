// Turns the diff ops into the rows the view draws, for either layout, with
// unchanged runs folded when only the changes are shown.

import type { DiffData, Span } from "../ipc";

export const EQUAL = 0;
export const DELETE = 1;
export const INSERT = 2;
/** Side by side only: an old line and a new line paired on one row. */
export const MODIFY = 3;
export const FOLD = 4;

/** Unchanged lines kept around each change when folding. */
export const CONTEXT = 3;

export interface Row {
  kind: number;
  /** Line index in A, or -1. */
  a: number;
  /** Line index in B, or -1. */
  b: number;
  wordsA?: Span[];
  wordsB?: Span[];
  /** For FOLD: index of the folded op and number of hidden lines. */
  op?: number;
  hidden?: number;
}

export interface RowModel {
  rows: Row[];
  /** Index of the first row of each change block. */
  changes: number[];
}

export function buildRows(
  diff: DiffData,
  unified: boolean,
  onlyChanges: boolean,
  expanded: ReadonlySet<number>,
): RowModel {
  const rows: Row[] = [];
  const changes: number[] = [];
  const { ops } = diff;

  for (let index = 0; index < ops.length; ) {
    const op = ops[index];
    if (op.kind === "equal") {
      const n = op.oldLen;
      let head = n;
      let tail = 0;
      if (onlyChanges && !expanded.has(index)) {
        head = index === 0 ? 0 : CONTEXT;
        tail = index === ops.length - 1 ? 0 : CONTEXT;
        // Folding a single line would not save any space.
        if (n - head - tail < 2) {
          head = n;
          tail = 0;
        }
      }
      const equal = (i: number) => rows.push({ kind: EQUAL, a: op.oldStart + i, b: op.newStart + i });
      for (let i = 0; i < head && i < n; i++) equal(i);
      if (head < n) {
        rows.push({ kind: FOLD, a: -1, b: -1, op: index, hidden: n - head - tail });
        for (let i = n - tail; i < n; i++) equal(i);
      }
      index++;
      continue;
    }

    // A change block: the ops up to the next unchanged run.
    let end = index;
    while (end < ops.length && ops[end].kind !== "equal") end++;
    const block = ops.slice(index, end);
    changes.push(rows.length);
    if (unified) {
      // All removed lines first, then all added ones, as in a unified diff.
      for (const op of block) {
        for (let i = 0; i < op.oldLen; i++) {
          rows.push({ kind: DELETE, a: op.oldStart + i, b: -1, wordsA: op.words?.[i]?.old });
        }
      }
      for (const op of block) {
        for (let i = 0; i < op.newLen; i++) {
          rows.push({ kind: INSERT, a: -1, b: op.newStart + i, wordsB: op.words?.[i]?.new });
        }
      }
    } else {
      for (const op of block) {
        // A replace has as many old lines as new ones, paired by offset.
        if (op.kind === "replace") {
          for (let i = 0; i < op.oldLen; i++) {
            rows.push({
              kind: MODIFY,
              a: op.oldStart + i,
              b: op.newStart + i,
              wordsA: op.words?.[i]?.old,
              wordsB: op.words?.[i]?.new,
            });
          }
        } else {
          for (let i = 0; i < op.oldLen; i++) rows.push({ kind: DELETE, a: op.oldStart + i, b: -1 });
          for (let i = 0; i < op.newLen; i++) rows.push({ kind: INSERT, a: -1, b: op.newStart + i });
        }
      }
    }
    index = end;
  }

  return { rows, changes };
}

/** End (exclusive) of the change block that starts at `start`. */
export function blockEnd(rows: Row[], start: number): number {
  let end = start;
  while (end < rows.length && rows[end].kind !== EQUAL && rows[end].kind !== FOLD) end++;
  return end;
}
