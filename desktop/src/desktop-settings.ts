import { backend, errorMessage } from "./api";
import { element, text } from "./dom";

export function showDesktopSettings(root: HTMLElement, run: (action: () => Promise<void>) => void): void {
  root.innerHTML = `
    <section class="panel desktop-panel" aria-labelledby="desktop-title">
      <h2 id="desktop-title">桌面行为</h2>
      <label class="check"><input id="autostart" type="checkbox" role="switch" disabled>开机自启动</label>
      <p>登录系统后在托盘中启动。关闭窗口后继续保持连接，可从托盘恢复。</p>
      <p id="startup-status" role="status" class="settings-status"></p>
      <button id="exit-app" type="button" class="exit-app">退出 ShuVPN</button>
    </section>`;
  const startup = element(root, "#autostart", HTMLInputElement);
  void backend.autostart().then((enabled) => {
    if (!root.isConnected) return;
    startup.checked = enabled;
    startup.disabled = false;
  }).catch((error: unknown) => {
    if (root.isConnected) text(root, "#startup-status", `无法读取启动项：${errorMessage(error)}`);
  });
  startup.onchange = () => {
    const desired = startup.checked;
    startup.disabled = true;
    run(async () => {
      try {
        await backend.setAutostart(desired);
        startup.checked = await backend.autostart();
        text(root, "#startup-status", startup.checked ? "已启用，下次登录系统时生效" : "已关闭开机自启动");
      } catch (error) {
        startup.checked = !desired;
        throw error;
      } finally { startup.disabled = false; }
    });
  };
  element(root, "#exit-app", HTMLButtonElement).onclick = () => run(backend.exit);
}
