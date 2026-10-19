export function element<T extends Element>(root: ParentNode, selector: string, kind: { new (...args: never[]): T }): T {
  const found = root.querySelector(selector);
  if (!(found instanceof kind)) throw new Error(`界面缺少控件：${selector}`);
  return found;
}

export function text(root: ParentNode, selector: string, content: string): void {
  const target = element(root, selector, HTMLElement);
  if (target.textContent !== content) target.textContent = content;
}

export function bytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / 1024 ** 2).toFixed(1)} MiB`;
}

export function applyTheme(mode: string): void {
  if (mode === "system") delete document.documentElement.dataset.theme;
  else if (document.documentElement.dataset.theme !== mode) document.documentElement.dataset.theme = mode;
  applyAppearance();
}
import { applyAppearance } from "./theme";

