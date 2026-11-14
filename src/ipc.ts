// Typed access to the backend commands and events.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Side = "a" | "b";

export type FileError =
  | { kind: "binary" }
  | { kind: "folder" }
  | { kind: "unreadable"; reason: string };

export interface FileView {
  path: string;
  name: string;
}

export interface Snapshot {
  files: [FileView | null, FileView | null];
  errors: [FileError | null, FileError | null];
  ignoreWhitespace: boolean;
  status: "idle" | "computing" | "ready" | "failed";
  /** Generation of the diff that `getDiff` returns, if any. */
  ready: number | null;
  /** Increases with every change; older snapshots can be ignored. */
  seq: number;
}

/** `[start, end)` in UTF-16 code units, as used by `String.slice`. */
export type Span = [number, number];

export interface WordChanges {
  old: Span[];
  new: Span[];
}

export interface Op {
  kind: "equal" | "delete" | "insert" | "replace";
  oldStart: number;
  oldLen: number;
  newStart: number;
  newLen: number;
  /** For `replace`: one entry per paired line. */
  words?: WordChanges[];
}

export type EolStyle = "none" | "lf" | "crlf" | "mixed";

export interface Stats {
  added: number;
  removed: number;
  /** From 0 to 1. */
  similarity: number;
  oldEol: EolStyle;
  newEol: EolStyle;
  eolDiffers: boolean;
}

export interface DiffData {
  generation: number;
  oldLines: string[];
  newLines: string[];
  ops: Op[];
  stats: Stats;
}

export interface Init {
  os: string;
  lang: string;
  effects: boolean;
  session: Snapshot;
}

export interface MenuState {
  unified: boolean;
  onlyChanges: boolean;
  ignoreWhitespace: boolean;
}

export const api = {
  init: () => invoke<Init>("init"),
  getDiff: () => invoke<DiffData | null>("get_diff"),
  openPaths: (paths: string[], side: Side | null) => invoke<void>("open_paths", { paths, side }),
  choose: (side: Side | null) => invoke<void>("choose", { side }),
  swap: () => invoke<void>("swap"),
  reset: () => invoke<void>("reset"),
  setIgnoreWhitespace: (value: boolean) => invoke<void>("set_ignore_whitespace", { value }),
  syncMenu: (state: MenuState) => invoke<void>("sync_menu", { state }),
};

export function onSession(handler: (snapshot: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("session", (event) => handler(event.payload));
}

/** Menu items that only change the view (macOS menu bar). */
export function onMenu(handler: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("menu", (event) => handler(event.payload));
}
