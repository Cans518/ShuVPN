import { backend } from "./api";
import type { Settings } from "./api";
import { applyTheme, element, text } from "./dom";
import { showAppearance } from "./appearance";
import { showDesktopSettings } from "./desktop-settings";
import { decorateIcons } from "./icons";

export function showSettings(root: HTMLElement, settings: Settings, run: (action: () => Promise<void>) => void): () => Promise<void> {
  root.innerHTML = `
    <header><h1>网络设置</h1><p>保存后在下一次连接时使用。</p></header>
    <form id="settings-form" class="panel form-grid">
      <label class="wide">VPN 网关<input name="server" required autocomplete="off"></label>
      <label>连接超时（秒）<input name="timeoutSeconds" type="number" min="1" max="120" required></label>
      <label>外观<select name="themeMode"><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></select></label>
      <fieldset class="wide"><legend>本机代理</legend><p>只有使用代理的应用会采用这些端口。端口为 0 时由系统分配。</p>
        <div class="form-grid">
          <label class="check"><input name="httpProxyEnabled" type="checkbox">启用 HTTP 代理</label>
          <label class="check"><input name="socksProxyEnabled" type="checkbox">启用 SOCKS5 代理</label>
          <label>HTTP 监听地址<input name="httpListen" required></label>
          <label>HTTP 端口<input name="httpPort" type="number" min="0" max="65535" required></label>
          <label>SOCKS5 监听地址<input name="socksListen" required></label>
          <label>SOCKS5 端口<input name="socksPort" type="number" min="0" max="65535" required></label>
          <label class="check wide"><input name="systemProxyEnabled" type="checkbox">连接时启用系统代理，断开时恢复</label>
        </div>
      </fieldset>
      <fieldset class="wide"><legend>桌面流量接管</legend>
        <label class="check"><input name="vpnEnabled" type="checkbox">连接时启动 TUN（可能需要系统授权）</label>
        <div class="form-grid">
          <label>捕获模式<select name="vpnCaptureMode"><option value="campus">校园分流</option><option value="full">完整捕获（实验性）</option></select></label>
          <label>MTU<input name="vpnMtu" type="number" min="1280" max="9000" required></label>
        </div><p>完整捕获将流量交给规则引擎；实际走校园还是直连仍由规则与校园资源授权决定。</p>
      </fieldset>
      <div class="wide actions"><button type="submit" class="primary">保存设置</button><span id="saved" role="status"></span></div>
    </form>
    <div id="appearance-settings"></div>
    <div id="desktop-settings"></div>`;
  const dispose = showAppearance(element(root, "#appearance-settings", HTMLDivElement), run);
  showDesktopSettings(element(root, "#desktop-settings", HTMLDivElement), run);
  decorateIcons(root);
  const form = element(root, "#settings-form", HTMLFormElement);
  let revision = 0;
  let saving = false;
  for (const [key, value] of Object.entries(settings)) {
    const field = form.elements.namedItem(key);
    if (field instanceof HTMLInputElement) {
      if (field.type === "checkbox") field.checked = value === true;
      else field.value = String(value);
    } else if (field instanceof HTMLSelectElement) field.value = String(value);
  }
  form.addEventListener("input", () => {
    revision++;
    text(root, "#saved", "有未保存的修改");
  });
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (saving) return;
    saving = true;
    const submittedRevision = revision;
    const save = element(form, 'button[type="submit"]', HTMLButtonElement);
    save.disabled = true;
    run(async () => {
      try {
        const edits = [...form.elements].flatMap((field) => {
          if (!(field instanceof HTMLInputElement || field instanceof HTMLSelectElement)) return [];
          const value = field instanceof HTMLInputElement && field.type === "checkbox" ? field.checked
            : field instanceof HTMLInputElement && field.type === "number" ? field.valueAsNumber : field.value;
          return [{ key: field.name, value }];
        });
        const saved = await backend.saveSettings(edits);
        applyTheme(saved.themeMode);
        text(root, "#saved", submittedRevision === revision
          ? `已保存 · 修订 ${saved.revision}` : "前一份修改已保存，当前还有未保存的修改");
      } finally {
        saving = false;
        save.disabled = false;
      }
    });
  });
  return dispose;
}
