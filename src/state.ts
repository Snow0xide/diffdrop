// Application state. The backend owns the files and the diff; the view
// options that never leave the window live here.

import type { DiffData, Snapshot } from "./ipc";

export interface State {
  os: string;
  session: Snapshot;
  /** Latest diff received; kept while a newer one is being computed. */
  diff: DiffData | null;
  unified: boolean;
  onlyChanges: boolean;
}

type Listener = (state: State) => void;

const listeners: Listener[] = [];

export const state: State = {
  os: "",
  session: {
    files: [null, null],
    errors: [null, null],
    ignoreWhitespace: false,
    status: "idle",
    ready: null,
    seq: 0,
  },
  diff: null,
  unified: false,
  onlyChanges: false,
};

export function subscribe(listener: Listener): void {
  listeners.push(listener);
}

export function update(changes: Partial<State>): void {
  Object.assign(state, changes);
  for (const listener of listeners) listener(state);
}

/** The diff is shown once both files loaded; until then, the drop zones. */
export function showsDiff(s: State): boolean {
  return s.diff !== null && (s.session.status === "ready" || s.session.status === "computing");
}
