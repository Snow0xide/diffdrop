// The few Lucide icons the interface uses.

import {
  ArrowLeftRight,
  ChevronDown,
  ChevronsUpDown,
  ChevronUp,
  Columns2,
  createElement,
  FileText,
  FileUp,
  FoldVertical,
  LoaderCircle,
  RotateCcw,
  Rows2,
  Space,
  TriangleAlert,
} from "lucide";

const icons = {
  ArrowLeftRight,
  ChevronDown,
  ChevronsUpDown,
  ChevronUp,
  Columns2,
  FileText,
  FileUp,
  FoldVertical,
  LoaderCircle,
  RotateCcw,
  Rows2,
  Space,
  TriangleAlert,
};

export type IconName = keyof typeof icons;

export function icon(name: IconName, size = 16): SVGElement {
  return createElement(icons[name], {
    width: size,
    height: size,
    "stroke-width": 1.75,
    "aria-hidden": "true",
  });
}

/** Prepends the icon named in each `data-icon` attribute. */
export function applyIcons(root: ParentNode): void {
  for (const el of root.querySelectorAll<HTMLElement>("[data-icon]")) {
    el.prepend(icon(el.dataset.icon as IconName));
  }
}
