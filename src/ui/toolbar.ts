// Toolbar buttons. They only report clicks; their state comes from `render`.

import type { State } from "../state";

export interface ToolbarActions {
  swap(): void;
  reset(): void;
  setUnified(unified: boolean): void;
  toggleIgnoreWhitespace(): void;
  toggleOnlyChanges(): void;
  prev(): void;
  next(): void;
}

export class Toolbar {
  private readonly swap: HTMLButtonElement;
  private readonly reset: HTMLButtonElement;
  private readonly views: HTMLButtonElement[];
  private readonly ignoreWhitespace: HTMLButtonElement;
  private readonly onlyChanges: HTMLButtonElement;
  private readonly prev: HTMLButtonElement;
  private readonly next: HTMLButtonElement;

  constructor(root: HTMLElement, actions: ToolbarActions) {
    const button = (selector: string) => root.querySelector<HTMLButtonElement>(selector)!;
    this.swap = button("#swap");
    this.reset = button("#reset");
    this.views = [...root.querySelectorAll<HTMLButtonElement>("[data-view]")];
    this.ignoreWhitespace = button("#ignore-whitespace");
    this.onlyChanges = button("#only-changes");
    this.prev = button("#prev");
    this.next = button("#next");

    this.swap.addEventListener("click", actions.swap);
    this.reset.addEventListener("click", actions.reset);
    for (const view of this.views) {
      view.addEventListener("click", () => actions.setUnified(view.dataset.view === "unified"));
    }
    this.ignoreWhitespace.addEventListener("click", actions.toggleIgnoreWhitespace);
    this.onlyChanges.addEventListener("click", actions.toggleOnlyChanges);
    this.prev.addEventListener("click", actions.prev);
    this.next.addEventListener("click", actions.next);
  }

  render(state: State, comparing: boolean, changes: number): void {
    const anyFile = state.session.files.some((f) => f !== null);
    this.swap.disabled = !anyFile;
    this.reset.disabled = !anyFile;
    for (const view of this.views) {
      view.setAttribute("aria-checked", String((view.dataset.view === "unified") === state.unified));
    }
    this.ignoreWhitespace.setAttribute("aria-pressed", String(state.session.ignoreWhitespace));
    this.onlyChanges.setAttribute("aria-pressed", String(state.onlyChanges));
    this.prev.disabled = !comparing || changes === 0;
    this.next.disabled = !comparing || changes === 0;
  }
}
