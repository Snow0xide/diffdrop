import { setLang, translate } from "./i18n";
import { applyIcons } from "./icons";
import { api, onMenu, onSession, type Snapshot } from "./ipc";
import { showsDiff, state, subscribe, update } from "./state";
import { DiffView } from "./ui/diff-view";
import { DropZones } from "./ui/dropzones";
import { StatusBar } from "./ui/stats";
import { Toolbar } from "./ui/toolbar";

const byId = (id: string) => document.getElementById(id)!;

async function main(): Promise<void> {
  // Events can arrive while the window is still being set up; keep the latest.
  let started = false;
  let early: Snapshot | null = null;
  await onSession((snapshot) => (started ? applySnapshot(snapshot) : (early = snapshot)));

  const init = await api.init();
  setLang(init.lang);
  document.body.dataset.os = init.os;
  document.body.dataset.effects = String(init.effects);
  translate(document);
  applyIcons(document);

  const diffSection = byId("diff");
  const emptyMessage = byId("diff-empty");
  const view = new DiffView(byId("diff-scroll"), byId("diff-canvas"), byId("hbars"));
  const zones = new DropZones(byId("zones"), byId("drop-overlay"));
  const statusBar = new StatusBar(byId("statusbar"));
  const toolbar = new Toolbar(document.querySelector<HTMLElement>(".toolbar")!, {
    swap: () => api.swap(),
    reset: () => api.reset(),
    setUnified: (unified) => update({ unified }),
    toggleIgnoreWhitespace: () => api.setIgnoreWhitespace(!state.session.ignoreWhitespace),
    toggleOnlyChanges: () => update({ onlyChanges: !state.onlyChanges }),
    prev: () => view.prev(),
    next: () => view.next(),
  });

  // What the diff view currently shows, to redraw only when something changed.
  let shown = { diff: null as unknown, unified: false, onlyChanges: false, files: "" };

  subscribe((s) => {
    const comparing = showsDiff(s);
    zones.render(s.session, comparing);
    diffSection.hidden = !comparing;
    if (comparing && s.diff) {
      const files = s.session.files.map((f) => f?.path).join("\n");
      if (
        shown.diff !== s.diff ||
        shown.unified !== s.unified ||
        shown.onlyChanges !== s.onlyChanges ||
        shown.files !== files
      ) {
        view.show(s.diff, s.unified, s.onlyChanges, shown.files === files);
        shown = { diff: s.diff, unified: s.unified, onlyChanges: s.onlyChanges, files };
      }
      emptyMessage.hidden = view.changeCount > 0 || (!s.onlyChanges && view.rowCount > 0);
    } else {
      shown = { diff: null, unified: false, onlyChanges: false, files: "" };
    }
    toolbar.render(s, comparing, view.changeCount);
    statusBar.render(s, comparing);
    api.syncMenu({
      unified: s.unified,
      onlyChanges: s.onlyChanges,
      ignoreWhitespace: s.session.ignoreWhitespace,
    });
  });

  let request = 0;
  async function applySnapshot(session: Snapshot): Promise<void> {
    if (session.seq < state.session.seq) return;
    const id = ++request;
    let diff = state.diff;
    if (session.ready !== null && session.ready !== diff?.generation) {
      diff = await api.getDiff();
      if (id !== request) return; // a newer snapshot took over
    } else if (session.ready === null && session.status !== "computing") {
      diff = null;
    }
    update({ session, diff });
  }

  onMenu((id) => {
    switch (id) {
      case "view-split":
        return update({ unified: false });
      case "view-unified":
        return update({ unified: true });
      case "only-changes":
        return update({ onlyChanges: !state.onlyChanges });
      case "next-change":
        return view.next();
      case "prev-change":
        return view.prev();
    }
  });

  // ⌘ on macOS, Ctrl elsewhere. On macOS the menu bar owns ⌘O and ⌘⇧S.
  const mac = init.os === "macos";
  window.addEventListener("keydown", (e) => {
    if (!(mac ? e.metaKey : e.ctrlKey) || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === "arrowdown" && !e.shiftKey) view.next();
    else if (key === "arrowup" && !e.shiftKey) view.prev();
    else if (!mac && key === "s" && e.shiftKey) void api.swap();
    else if (!mac && key === "o" && !e.shiftKey) void api.choose(null);
    else return;
    e.preventDefault();
  });

  started = true;
  await applySnapshot(init.session);
  if (early) await applySnapshot(early);
}

void main();
