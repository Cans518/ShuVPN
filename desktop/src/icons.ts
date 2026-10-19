import {
  createElement, ArrowDown, ArrowUp, ChevronLeft, History, ListFilter,
  Power, Settings2, ShieldCheck, UserRound, Minus, X, Palette,
} from "lucide";

const icons = {
  down: ArrowDown,
  up: ArrowUp,
  back: ChevronLeft,
  history: History,
  rules: ListFilter,
  power: Power,
  settings: Settings2,
  shield: ShieldCheck,
  account: UserRound,
  minimize: Minus,
  close: X,
  palette: Palette,
};

function isIconName(value: string): value is keyof typeof icons {
  return Object.hasOwn(icons, value);
}

export function decorateIcons(root: ParentNode): void {
  for (const placeholder of root.querySelectorAll<HTMLElement>("[data-icon]")) {
    const name = placeholder.dataset.icon;
    if (!name || !isIconName(name)) continue;
    placeholder.replaceChildren(createElement(icons[name], {
      width: "20",
      height: "20",
      "stroke-width": "1.8",
      "aria-hidden": "true",
      focusable: "false",
    }));
  }
}
