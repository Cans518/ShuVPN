import "@picocss/pico/css/pico.css";
import "./style.css";
import thirdPartyNotices from "../THIRD_PARTY_NOTICES.md?raw";
import packageInfo from "../package.json";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { loadAppearance } from "./theme";
import { backend, errorMessage } from "./api";
import type { DesktopSnapshot } from "./api";
import { applyTheme, bytes, element, text } from "./dom";
import { showSettings } from "./settings-page";
import { showRouting } from "./routing-page";
import { decorateIcons } from "./icons";
import { showRecords } from "./records-page";

const app = element(document, "#app", HTMLDivElement);
app.innerHTML = `
  <a class="skip-link" href="#content">跳到内容</a>
  <header class="titlebar" data-tauri-drag-region>
    <span class="titlebar-brand" data-tauri-drag-region><span data-icon="shield"></span>ShuVPN <small data-tauri-drag-region>校园网络</small></span>
    <div class="window-controls">
      <button id="minimize-window" aria-label="最小化窗口" title="最小化"><span data-icon="minimize"></span></button>
      <button id="close-window" aria-label="关闭窗口并保留托盘" title="关闭窗口，继续在托盘运行"><span data-icon="close"></span></button>
    </div>
  </header>
  <header class="toolbar">
    <nav aria-label="主导航">
      <button data-page="connect" aria-current="page" aria-label="连接" title="连接"><span data-icon="back"></span><span>连接</span></button>
      <button data-page="account" aria-label="校园账户" title="校园账户"><span data-icon="account"></span><span>账户</span></button>
      <button data-page="routing" aria-label="规则分流" title="规则分流"><span data-icon="rules"></span><span>分流</span></button>
      <button data-page="records" aria-label="连接记录" title="连接记录"><span data-icon="history"></span><span>记录</span></button>
      <button data-page="settings" aria-label="网络设置" title="网络设置"><span data-icon="settings"></span><span>设置</span></button>
    </nav>
  </header>
  <main>
    <div id="failure" role="alert" hidden></div>
    <div id="content" tabindex="-1"></div>
  </main>
  <footer class="app-footer"><span id="app-state">正在启动</span><div id="feedback" role="status" aria-live="polite" hidden></div><details><summary>${packageInfo.version} · 开源许可</summary><pre id="third-party-notices"></pre></details></footer>`;
text(app, "#third-party-notices", thirdPartyNotices);
decorateIcons(app);

let currentPage = "connect";
let snapshot: DesktopSnapshot | null = null;
let pageRevision = 0;
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let disposePage: (() => Promise<void>) | undefined;
let feedbackTimer: ReturnType<typeof setTimeout> | undefined;
const pending = new Set<() => Promise<void>>();
let accountRequests = 0;
let qrImageUrl: string | undefined;
let connectionSubmitting = false;

function clearQrImage(): void {
  if (qrImageUrl) URL.revokeObjectURL(qrImageUrl);
  qrImageUrl = undefined;
}

function report(error: unknown): void {
  const banner = element(app, "#failure", HTMLDivElement);
  banner.textContent = errorMessage(error);
  banner.hidden = false;
}

function run(action: () => Promise<void>): void {
  if (pending.has(action)) return;
  pending.add(action);
  const feedback = element(app, "#feedback", HTMLDivElement);
  clearTimeout(feedbackTimer);
  feedback.hidden = false;
  feedback.textContent = "正在处理…";
  element(app, "#failure", HTMLDivElement).hidden = true;
  void action().then(async () => {
    await refresh();
    feedback.textContent = "已完成";
    feedbackTimer = setTimeout(() => {
      if (pending.size === 0) feedback.hidden = true;
    }, 1800);
  }).catch((error: unknown) => {
    feedback.hidden = true;
    report(error);
  }).finally(() => {
    pending.delete(action);
  });
}

function runAccount(action: () => Promise<void>): void {
  if (accountRequests > 0) return;
  accountRequests++;
  if (snapshot && currentPage === "account") {
    updateAccount(element(app, "#content", HTMLDivElement), snapshot);
  }
  run(async () => {
    try { await action(); } finally { accountRequests--; }
  });
}

function navigate(page: string): void {
  clearTimeout(feedbackTimer);
  element(app, "#feedback", HTMLDivElement).hidden = true;
  element(app, "#failure", HTMLDivElement).hidden = true;
  void showPage(page).catch(report);
}

const phaseLabels = {
  disconnected: "尚未连接",
  connecting: "正在建立连接",
  connected: "已连接校园网络",
  disconnecting: "正在断开并清理",
  failed: "连接需要处理",
};

function update(current: DesktopSnapshot): void {
  snapshot = current;
  applyTheme(current.settings.themeMode);
  app.dataset.phase = current.connection.phase;
  text(app, "#app-state", phaseLabels[current.connection.phase]);
  const root = element(app, "#content", HTMLDivElement);
  if (currentPage === "connect") {
    text(root, "#connection-state", phaseLabels[current.connection.phase]);
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
  if (currentPage === "account") updateAccount(root, current);
}

function updateAccount(root: HTMLElement, current: DesktopSnapshot): void {
  const account = current.account;
  const busy = account.busy || accountRequests > 0;
  text(root, "#account-state", account.signedIn
    ? account.profile?.name ?? account.accountId ?? "已登录"
    : "尚未登录");
  text(root, "#account-message", account.progressLabel ?? account.errorMessage
    ?? (account.sessionExpired ? "会话已过期，请重新登录" : "账户会话保存在本机，可随时退出登录。"));
  element(root, "#login-form", HTMLFormElement).hidden = account.signedIn;
  element(root, "#sign-out", HTMLButtonElement).hidden = !account.signedIn;
  element(root, "#sign-in", HTMLButtonElement).disabled = busy;
  const qr = element(root, "#qr-sign-in", HTMLButtonElement);
  qr.hidden = account.signedIn;
  qr.disabled = busy;
  element(root, "#cancel-account", HTMLButtonElement).hidden = !busy && !account.verification;
  const verification = element(root, "#verification", HTMLFormElement);
  verification.hidden = !account.verification;
  if (account.verification) {
    const method = element(root, "#method", HTMLSelectElement);
    const signature = JSON.stringify(account.verification.methods);
    if (method.dataset.methods !== signature) {
      method.replaceChildren(...account.verification.methods.map((target) => {
        const option = document.createElement("option");
        option.value = target.method;
        option.textContent = `${target.method} · ${target.target}`;
        return option;
      }));
      method.value = account.verification.selectedMethod;
      method.dataset.methods = signature;
    }
    const send = element(root, "#send-code", HTMLButtonElement);
    const remaining = account.verification.resendAfterSeconds;
    send.disabled = remaining > 0 || busy;
    send.textContent = remaining > 0 ? `${remaining} 秒后重新发送` : "发送验证码";
    element(root, "#verify", HTMLButtonElement).disabled = busy;
  }
  const credentials = element(root, "#credentials", HTMLUListElement);
  credentials.replaceChildren(...account.credentials.map((credential) => {
    const item = document.createElement("li");
    item.textContent = `${credential.systemId} · ${credential.state}${credential.errorMessage ? ` · ${credential.errorMessage}` : ""}`;
    return item;
  }));
}

async function showPage(page: string): Promise<void> {
  if (!snapshot) return;
  const revision = ++pageRevision;
  const dispose = disposePage;
  disposePage = undefined;
  if (dispose) await dispose();
  if (revision !== pageRevision) return;
  if (currentPage === "account" && (accountRequests > 0 || snapshot.account.busy || snapshot.account.verification)) {
    await backend.cancelAccount();
  }
  if (revision !== pageRevision) return;
  clearQrImage();
  currentPage = page;
  for (const button of app.querySelectorAll<HTMLButtonElement>("nav button")) {
    if (button.dataset.page === page) button.setAttribute("aria-current", "page");
    else button.removeAttribute("aria-current");
  }
  // Replace the node so an older asynchronous page cannot overwrite a new page.
  const previous = element(app, "#content", HTMLDivElement);
  const root = document.createElement("div");
  root.id = "content";
  root.tabIndex = -1;
  root.dataset.page = page;
  previous.replaceWith(root);
  element(app, "main", HTMLElement).scrollTop = 0;
  if (page === "settings") disposePage = showSettings(root, snapshot.settings, run);
  else if (page === "routing") {
    const dispose = await showRouting(root, run);
    if (revision !== pageRevision) await dispose();
    else disposePage = dispose;
  }
  else if (page === "account") showAccount(root);
  else if (page === "records") {
    await showRecords(root, run);
  } else {
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
      update(snapshot);
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
          if (snapshot) update(snapshot);
        }
      });
    };
    update(snapshot);
    const interfaces = await backend.interfaces().catch((error: unknown) => {
      report(error);
      return [];
    });
    if (revision !== pageRevision) return;
    const select = element(root, "#interface", HTMLSelectElement);
    for (const network of interfaces) {
      const option = document.createElement("option");
      option.value = network.name;
      option.textContent = `${network.label || network.name}${network.recommended ? "（推荐）" : ""}`;
      select.append(option);
    }
  }
  if (revision === pageRevision && snapshot) {
    update(snapshot);
    root.focus({ preventScroll: true });
  }
}

function showAccount(root: HTMLElement): void {
  root.innerHTML = `
    <header><h1>校园账户</h1><p>使用上海大学统一身份认证账户。</p></header>
    <section class="panel account-panel">
      <h2 id="account-state">尚未登录</h2><p id="account-message" role="status"></p>
      <form id="login-form">
        <label>学工号<input id="username" autocomplete="username" required maxlength="128"></label>
        <label>密码<input id="password" type="password" autocomplete="current-password" required maxlength="1024"></label>
        <button id="sign-in" class="primary" type="submit">登录</button>
      </form>
      <button id="qr-sign-in" class="qr-sign-in" type="button">使用企业微信扫码登录</button>
      <figure id="qr-display" hidden>
        <img id="qr-image" alt="企业微信登录二维码" width="192" height="192">
        <figcaption>请使用企业微信扫码，并在手机上确认。</figcaption>
      </figure>
      <form id="verification" hidden>
        <h3>身份核验</h3>
        <label>验证方式<select id="method"></select></label>
        <button id="send-code" type="button">发送验证码</button>
        <label>验证码<input id="code" autocomplete="one-time-code" required maxlength="128"></label>
        <button id="verify" class="primary" type="submit">验证并完成登录</button>
      </form>
      <div class="actions"><button id="cancel-account" hidden>取消登录</button><button id="sign-out" hidden>退出账户</button></div>
      <ul id="credentials"></ul>
    </section>`;
  element(root, "#login-form", HTMLFormElement).onsubmit = (event) => {
    event.preventDefault();
    const password = element(root, "#password", HTMLInputElement);
    const secret = password.value;
    password.value = "";
    const username = element(root, "#username", HTMLInputElement).value;
    runAccount(async () => { await backend.signIn(username, secret); });
  };
  element(root, "#send-code", HTMLButtonElement).onclick = () => runAccount(async () => {
    await backend.sendCode(element(root, "#method", HTMLSelectElement).value);
  });
  element(root, "#qr-sign-in", HTMLButtonElement).onclick = () => runAccount(async () => {
    try {
      const available = await backend.signInQr((event) => {
        if (!root.isConnected) return;
        const display = element(root, "#qr-display", HTMLElement);
        clearQrImage();
        display.hidden = event.phase !== "waiting" || !event.image;
        if (event.phase === "waiting" && event.image) {
          qrImageUrl = URL.createObjectURL(new Blob([new Uint8Array(event.image)], { type: "image/png" }));
          element(root, "#qr-image", HTMLImageElement).src = qrImageUrl;
        }
      });
      if (!available) throw new Error("登录已完成，但校园服务凭据暂不可用，请查看账户状态后重试。");
    } finally {
      clearQrImage();
      element(root, "#qr-display", HTMLElement).hidden = true;
    }
  });
  element(root, "#verification", HTMLFormElement).onsubmit = (event) => {
    event.preventDefault();
    const code = element(root, "#code", HTMLInputElement);
    const submitted = code.value;
    code.value = "";
    runAccount(async () => { await backend.verifyCode(element(root, "#method", HTMLSelectElement).value, submitted); });
  };
  element(root, "#cancel-account", HTMLButtonElement).onclick = () => run(backend.cancelAccount);
  element(root, "#sign-out", HTMLButtonElement).onclick = () => run(async () => {
    const diagnostic = await backend.signOut();
    if (diagnostic) report(diagnostic);
  });
}

async function refresh(): Promise<void> {
  update(await backend.snapshot());
}

async function poll(): Promise<void> {
  try { await refresh(); } catch (error) { report(error); }
  pollTimer = setTimeout(() => { void poll(); }, 1000);
}

for (const button of app.querySelectorAll<HTMLButtonElement>("nav button")) {
  button.onclick = () => navigate(button.dataset.page ?? "connect");
}
element(app, "#minimize-window", HTMLButtonElement).onclick = () => { void getCurrentWindow().minimize().catch(report); };
element(app, "#close-window", HTMLButtonElement).onclick = () => { void backend.hideWindow().catch(report); };

window.addEventListener("pagehide", () => { clearTimeout(pollTimer); clearTimeout(feedbackTimer); });

async function start(): Promise<void> {
  await listen("shutdown-error", (event) => report(event.payload));
  await listen<string>("navigate", (event) => {
    if (["connect", "account", "routing", "records", "settings"].includes(event.payload)) navigate(event.payload);
  });
  await loadAppearance();
  snapshot = await backend.snapshot();
  await showPage("connect");
  await poll();
}

void start().catch(report);
