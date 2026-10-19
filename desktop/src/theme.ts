import { backend } from "./api";
import type { Appearance, ThemeColors } from "./api";

export const palettes = [
  { id: "ocean", name: "晴海", color: "#2864ad", dark: "#99c2ff" },
  { id: "jade", name: "青竹", color: "#19745d", dark: "#79d8b6" },
  { id: "iris", name: "暮紫", color: "#7753a6", dark: "#c5a7f3" },
  { id: "amber", name: "暖砂", color: "#9b5d21", dark: "#efbe84" },
];

let saved: Appearance | undefined;
let preview: Appearance | undefined;
let previewRevision = 0;
let saveRevision = 0;
const darkPreference = matchMedia("(prefers-color-scheme: dark)");
darkPreference.addEventListener("change", () => applyAppearance());

export function currentAppearance(): Appearance | undefined {
  return saved;
}

export function previewAppearance(appearance?: Appearance): void {
  previewRevision++;
  preview = appearance;
  applyAppearance();
}

export async function loadAppearance(): Promise<void> {
  saved = await backend.appearance();
  applyAppearance();
}

export async function saveAppearance(appearance: Appearance): Promise<void> {
  const revision = ++saveRevision;
  const submittedPreview = previewRevision;
  const result = await backend.saveAppearance(appearance);
  if (revision !== saveRevision) return;
  saved = result;
  if (submittedPreview === previewRevision) preview = undefined;
  applyAppearance();
}

function accentTextColor(accent: string): string {
  const linearChannel = (offset: number): number => {
    const channel = Number.parseInt(accent.slice(offset, offset + 2), 16) / 255;
    return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4;
  };
  const luminance = linearChannel(1) * .2126 + linearChannel(3) * .7152 + linearChannel(5) * .0722;
  return luminance > .179 ? "#000000" : "#ffffff";
}

export function applyAppearance(): void {
  const appearance = preview ?? saved;
  if (!appearance) return;
  const root = document.documentElement;
  const dark = root.dataset.theme === "dark" || (!root.dataset.theme && darkPreference.matches);
  const keys: (keyof ThemeColors)[] = ["accent", "canvas", "surface", "ink", "muted", "border"];
  for (const key of keys) root.style.removeProperty(`--${key}`);
  root.style.removeProperty("--on-accent");
  root.dataset.palette = appearance.palette;
  if (appearance.palette === "custom") {
    const colors = appearance.customTheme[dark ? "dark" : "light"];
    for (const key of keys) root.style.setProperty(`--${key}`, colors[key]);
    root.style.setProperty("--on-accent", accentTextColor(colors.accent));
  } else {
    const palette = palettes.find((palette) => palette.id === appearance.palette) ?? palettes[0];
    if (palette) {
      root.style.setProperty("--accent", dark ? palette.dark : palette.color);
      root.style.setProperty("--canvas", `color-mix(in srgb, var(--accent) 5%, ${dark ? "#141c29" : "#f8fafc"})`);
      root.style.setProperty("--surface", dark ? "color-mix(in srgb, var(--accent) 4%, #1b2433)" : "#ffffff");
      root.style.setProperty("--border", `color-mix(in srgb, var(--accent) 12%, ${dark ? "#344055" : "#e3e8f0"})`);
    }
  }
}
