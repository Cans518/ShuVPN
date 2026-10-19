import { backend } from "./api";
import type { DesktopSnapshot } from "./api";
import { bytes, element, text } from "./dom";
import { decorateIcons } from "./icons";

export const connectionPhaseLabels = {
  disconnected: "尚未连接",
  connecting: "正在建立连接",
  connected: "已连接校园网络",
  disconnecting: "正在断开并清理",
  failed: "连接需要处理",
};

export function createConnectionPage({ run, report, navigate, refreshView }: {
  run: (action: () => Promise<void>) => void;
  report: (error: unknown) => void;
  navigate: (page: string) => void;
  refreshView: () => void;
}) {
  let snapshot: DesktopSnapshot | null = null;
  let connectionSubmitting = false;

  function updateConnection(root: HTMLElement, current: DesktopSnapshot): void {
    snapshot = current;
    text(root, "#connection-state", connectionPhaseLabels[current.connection.phase]);
    text(root, "#gateway", current.settings.server);
    text(root, "#connection-message", current.connection.errorMessage ?? "");
    const warnings = current.connection.network?.warnings ?? [];
    const attention = Boolean(current.connection.errorMessage || warnings.length);
    element(root, ".connection-details", HTMLElement).dataset.attention = String(attention);
    text(root, "#connection-details-label", attention ? "连接详情 · 有提示" : "连接详情");
    const toggle = element(root, "#connect", HTMLButtonElement);
    const active = current.connection.phase === "connected" || current.connection.phase === "connecting";
    text(root, "#connect-label", current.connection.phase === "connecting" ? "取消连接"
      : active ? "断开连接" : current.connection.cleanupPending ? "重试清理"
      : current.account.signedIn ? "连接校园网络" : "登录账户");
    toggle.disabled = current.connection.phase === "disconnecting"
      || (connectionSubmitting && current.connection.phase !== "connecting");
    toggle.dataset.loading = String(current.connection.phase === "connecting");
    text(root, "#account-hint", current.account.signedIn
      ? current.account.profile?.name ?? current.account.accountId ?? "账户已登录" : "使用校园账户登录后连接");
    const accountHint = element(root, "#account-hint", HTMLElement);
    accountHint.title = accountHint.textContent ?? "";
    text(root, "#upload", bytes(current.connection.network?.uploadBytes ?? 0));
    text(root, "#download", bytes(current.connection.network?.downloadBytes ?? 0));
    text(root, "#virtual-address", current.connection.network?.virtualAddress ?? "等待连接");
    text(root, "#warnings", current.connection.network?.warnings.join("\n") ?? "");
    const locked = active || current.connection.phase === "disconnecting" || current.connection.cleanupPending;
    for (const protocol of ["http", "socks"] as const) {
      const control = element(root, `#${protocol}-enabled`, HTMLInputElement);
      control.checked = protocol === "http" ? current.settings.httpProxyEnabled : current.settings.socksProxyEnabled;
      control.disabled = locked;
      const address = protocol === "http" ? current.settings.httpListen : current.settings.socksListen;
      const port = protocol === "http" ? current.settings.httpPort : current.settings.socksPort;
      const endpoint = protocol === "http" ? current.connection.network?.httpEndpoint : current.connection.network?.socksEndpoint;
      text(root, `#${protocol}-endpoint`, current.connection.cleanupPending ? "清理尚未完成"
        : !control.checked ? "未启用"
        : active ? endpoint ?? "未启动，请查看错误信息"
        : `${address.includes(":") ? `[${address}]` : address}:${port || "自动"} · 连接后启用`);
    }
  }

  async function showConnection(root: HTMLElement, current: DesktopSnapshot): Promise<void> {
    root.innerHTML = `
      <section class="connection-card" aria-labelledby="connection-state">
        <p class="protocol-tag">aTrust <span>校园网络</span></p>
        <button id="connect" class="power-button"><span data-icon="power"></span><span id="connect-label">登录账户</span></button>
        <h1 id="connection-state" role="status" aria-live="polite">尚未连接</h1>
        <p id="account-hint"></p>
        <div class="traffic-summary" aria-label="累计流量">
          <span><span data-icon="up"></span><span id="upload">0 B</span></span>
          <span><span data-icon="down"></span><span id="download">0 B</span></span>
        </div>
      </section>
      <section class="proxy-panel" aria-label="连接时启用的本机代理">
        <div class="proxy-row"><label for="http-enabled"><strong>HTTP</strong><small id="http-endpoint"></small></label><input id="http-enabled" type="checkbox" role="switch" aria-label="连接时启用 HTTP 代理"></div>
        <div class="proxy-row"><label for="socks-enabled"><strong>SOCKS5</strong><small id="socks-endpoint"></small></label><input id="socks-enabled" type="checkbox" role="switch" aria-label="连接时启用 SOCKS5 代理"></div>
      </section>
      <details class="connection-details"><summary id="connection-details-label">连接详情</summary>
        <p id="connection-message" class="error"></p>
        <p id="warnings" class="error"></p>
        <dl><dt>网关</dt><dd id="gateway"></dd><dt>虚拟地址</dt><dd id="virtual-address">等待连接</dd></dl>
        <label>本次 TUN 物理网卡
          <select id="interface"><option value="">自动选择</option></select></label>
          <label>物理 DNS 上游（可选）<input id="dns" placeholder="自动读取物理接口" autocomplete="off"></label>
      </details>`;
    decorateIcons(root);
    for (const protocol of ["http", "socks"] as const) {
      element(root, `#${protocol}-enabled`, HTMLInputElement).onchange = () => run(async () => {
        await backend.saveSettings([{
          key: protocol === "http" ? "httpProxyEnabled" : "socksProxyEnabled",
          value: element(root, `#${protocol}-enabled`, HTMLInputElement).checked,
        }]);
      });
    }
    element(root, "#connect", HTMLButtonElement).onclick = () => {
      if (!snapshot || (connectionSubmitting && snapshot.connection.phase !== "connecting")) return;
      const stop = snapshot.connection.phase === "connected"
        || snapshot.connection.phase === "connecting" || snapshot.connection.cleanupPending;
      if (!stop && !snapshot.account.signedIn) {
        navigate("account");
        return;
      }
      connectionSubmitting = true;
      refreshView();
      run(async () => {
        try {
          if (stop) {
            const diagnostic = await backend.disconnect();
            if (diagnostic) report(diagnostic);
          } else {
            await backend.connect(
              element(root, "#interface", HTMLSelectElement).value,
              element(root, "#dns", HTMLInputElement).value || null,
            );
          }
        } finally {
          connectionSubmitting = false;
          if (snapshot) refreshView();
        }
      });
    };
    updateConnection(root, current);
    const interfaces = await backend.interfaces().catch((error: unknown) => {
      report(error);
      return [];
    });
    if (!root.isConnected) return;
    const select = element(root, "#interface", HTMLSelectElement);
    for (const network of interfaces) {
      const option = document.createElement("option");
      option.value = network.name;
      option.textContent = `${network.label || network.name}${network.recommended ? "（推荐）" : ""}`;
      select.append(option);
    }
  }

  return { show: showConnection, update: updateConnection };
}
