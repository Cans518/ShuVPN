import { backend } from "./api";
import type { DesktopSnapshot } from "./api";
import { element, text } from "./dom";

export function createAccountPage(
  run: (action: () => Promise<void>) => void,
  report: (error: unknown) => void,
) {
  let snapshot: DesktopSnapshot | null = null;
  let activeRoot: HTMLElement | undefined;
  let accountRequests = 0;
  let qrImageUrl: string | undefined;

  function clearQrImage(): void {
    if (qrImageUrl) URL.revokeObjectURL(qrImageUrl);
    qrImageUrl = undefined;
  }

  function runAccount(action: () => Promise<void>): void {
    if (accountRequests > 0) return;
    accountRequests++;
    if (snapshot && activeRoot?.isConnected) {
      updateAccount(activeRoot, snapshot);
    }
    run(async () => {
      try { await action(); } finally { accountRequests--; }
    });
  }

  function updateAccount(root: HTMLElement, current: DesktopSnapshot): void {
    snapshot = current;
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

  function showAccount(root: HTMLElement): void {
    activeRoot = root;
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

  async function cancelWork(current: DesktopSnapshot): Promise<void> {
    if (accountRequests > 0 || current.account.busy || current.account.verification) {
      await backend.cancelAccount();
    }
  }

  function dispose(): void {
    clearQrImage();
    activeRoot = undefined;
  }

  return { show: showAccount, update: updateAccount, cancelWork, dispose };
}
