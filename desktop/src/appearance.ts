import type { Appearance, CustomTheme, ThemeColors } from "./api";
import { element, text } from "./dom";
import { currentAppearance, palettes, previewAppearance, saveAppearance } from "./theme";

function parseTheme(value: unknown): CustomTheme {
  if (typeof value !== "object" || value === null || !("name" in value)
      || typeof value.name !== "string" || !value.name.trim() || value.name.length > 32) {
    throw new Error("主题需要 1 至 32 个字符的名称。");
  }
  const colors = (value: unknown): ThemeColors => {
    if (typeof value !== "object" || value === null) throw new Error("主题需要 light 和 dark 配色。");
    const color = (key: string): string => {
      const entry: unknown = Reflect.get(value, key);
      if (typeof entry !== "string" || !/^#[\da-f]{6}$/i.test(entry)) throw new Error(`${key} 需要 #RRGGBB 颜色。`);
      return entry;
    };
    return { accent: color("accent"), canvas: color("canvas"), surface: color("surface"),
      ink: color("ink"), muted: color("muted"), border: color("border") };
  };
  return { name: value.name, light: colors(Reflect.get(value, "light")), dark: colors(Reflect.get(value, "dark")) };
}

export function showAppearance(root: HTMLElement, run: (action: () => Promise<void>) => void): () => Promise<void> {
  const initial = currentAppearance();
  if (!initial) throw new Error("外观尚未加载");
  let selectedPalette = initial.palette;
  let editRevision = 0;
  let importRevision = 0;
  root.innerHTML = `
    <section class="panel appearance-panel" aria-labelledby="appearance-title">
      <h2 id="appearance-title">你的配色</h2>
      <p>五种内置配色，支持浅色、深色与跟随系统。</p>
      <section class="palette-list" aria-label="配色方案">
        ${palettes.map((palette) => `<button type="button" data-palette="${palette.id}" aria-pressed="false"><span class="swatch" style="--swatch:${palette.color}"></span>${palette.name}</button>`).join("")}
        <button type="button" data-palette="custom" aria-pressed="false"><span data-icon="palette"></span>自定义</button>
      </section>
      <details class="theme-editor"><summary>编辑或导入自定义主题</summary>
        <form id="theme-form">
          <label>主题名称<input id="theme-name" maxlength="32" required></label>
          <div class="theme-color-groups">
            ${(["light", "dark"] as const).map((mode) => `<fieldset><legend>${mode === "light" ? "浅色" : "深色"}</legend>
              ${Object.entries({ accent: "主题色", canvas: "背景", surface: "卡片", ink: "正文", muted: "次要文字", border: "边框" }).map(([key, name]) =>
                `<label class="color-field">${name}<input type="color" data-mode="${mode}" data-color="${key}" aria-label="${mode === "light" ? "浅色" : "深色"}${name}"></label>`).join("")}
            </fieldset>`).join("")}
          </div>
          <p class="theme-hint">编辑时实时预览；请保持文字与背景清晰可辨。保存后才会在下次启动时使用。</p>
          <div class="actions"><button type="submit" class="primary">保存自定义主题</button><button id="theme-reset" type="button">撤销预览</button></div>
        </form>
        <div class="actions"><button id="theme-export" type="button">导出 JSON</button><label class="import-theme">导入 JSON<input id="theme-import" type="file" accept=".json,application/json"></label></div>
      </details>
      <p id="theme-status" role="status" class="settings-status"></p>
    </section>`;
  const markSelected = () => {
    for (const button of root.querySelectorAll<HTMLButtonElement>("[data-palette]")) {
      button.setAttribute("aria-pressed", String(button.dataset.palette === selectedPalette));
    }
  };
  const populate = (theme: CustomTheme) => {
    element(root, "#theme-name", HTMLInputElement).value = theme.name;
    for (const field of root.querySelectorAll<HTMLInputElement>("[data-color]")) {
      const mode = field.dataset.mode === "dark" ? "dark" : "light";
      const key = field.dataset.color;
      if (key && Object.hasOwn(theme[mode], key)) field.value = Reflect.get(theme[mode], key);
    }
  };
  const draft = (): CustomTheme => {
    const theme = structuredClone(initial.customTheme);
    theme.name = element(root, "#theme-name", HTMLInputElement).value;
    for (const field of root.querySelectorAll<HTMLInputElement>("[data-color]")) {
      const mode = field.dataset.mode === "dark" ? "dark" : "light";
      if (field.dataset.color) Reflect.set(theme[mode], field.dataset.color, field.value);
    }
    return parseTheme(theme);
  };
  const readDraft = (): CustomTheme | undefined => {
    try { return draft(); } catch (error) {
      text(root, "#theme-status", error instanceof Error ? error.message : "主题无效");
      return undefined;
    }
  };
  const previewDraft = () => {
    editRevision++;
    try {
      selectedPalette = "custom";
      previewAppearance({ palette: "custom", customTheme: draft() });
      markSelected();
      text(root, "#theme-status", "正在预览，尚未保存");
    } catch (error) {
      text(root, "#theme-status", error instanceof Error ? error.message : "主题无效");
    }
  };
  let saving = false;
  const save = (appearance: Appearance) => {
    if (saving) return;
    saving = true;
    importRevision++;
    const submittedRevision = editRevision;
    for (const button of root.querySelectorAll<HTMLButtonElement>("[data-palette], #theme-form button")) button.disabled = true;
    run(async () => {
      try {
        await saveAppearance(appearance);
        if (root.isConnected) {
          if (submittedRevision === editRevision) {
            selectedPalette = appearance.palette;
            markSelected();
            text(root, "#theme-status", "配色已保存");
          } else {
            text(root, "#theme-status", "前一份配色已保存，当前还有未保存的编辑");
          }
        }
      } finally {
        saving = false;
        for (const button of root.querySelectorAll<HTMLButtonElement>("[data-palette], #theme-form button")) button.disabled = false;
      }
    });
  };
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-palette]")) {
    button.onclick = () => {
      const palette = button.dataset.palette ?? "ocean";
      const customTheme = palette === "custom" ? readDraft() : currentAppearance()?.customTheme;
      if (customTheme) save({ palette, customTheme });
    };
  }
  const form = element(root, "#theme-form", HTMLFormElement);
  form.addEventListener("input", previewDraft);
  form.onsubmit = (event) => {
    event.preventDefault();
    const customTheme = readDraft();
    if (customTheme) save({ palette: "custom", customTheme });
  };
  element(root, "#theme-reset", HTMLButtonElement).onclick = () => {
    importRevision++;
    const saved = currentAppearance();
    if (saved) { populate(saved.customTheme); selectedPalette = saved.palette; }
    previewAppearance();
    markSelected();
    text(root, "#theme-status", "已恢复保存的配色");
  };
  element(root, "#theme-export", HTMLButtonElement).onclick = () => {
    const theme = readDraft();
    if (!theme) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(theme, null, 2)], { type: "application/json" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "shuvpn-theme.json";
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  element(root, "#theme-import", HTMLInputElement).onchange = (event) => {
    if (!(event.currentTarget instanceof HTMLInputElement)) return;
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) return;
    const importing = ++importRevision;
    const beforeRead = editRevision;
    run(async () => {
      if (file.size > 16384) throw new Error("主题文件不能超过 16 KiB。");
      const theme = parseTheme(JSON.parse(await file.text()));
      if (!root.isConnected || importing !== importRevision || beforeRead !== editRevision) return;
      populate(theme);
      previewDraft();
    });
  };
  populate(initial.customTheme);
  markSelected();
  return async () => { importRevision++; previewAppearance(); };
}
