import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";

if (process.platform !== "win32") throw new Error("This smoke driver requires Windows WebView2.");
const [executableArgument, reportArgument] = process.argv.slice(2);
if (!executableArgument || !reportArgument) {
  throw new Error("Usage: node tool/tauri_windows_smoke.mjs ABSOLUTE_EXE NEW_REPORT_DIRECTORY");
}
const executable = path.resolve(executableArgument);
const report = path.resolve(reportArgument);
await mkdir(report, { recursive: false });
const profile = path.join(report, "profile");
const productHash = createHash("sha256").update(await readFile(executable)).digest("hex");
const expectedVersion = JSON.parse(await readFile(path.resolve("desktop/package.json"), "utf8")).version;
const runs = [];
const nativeTray = process.argv.includes("--native-tray");
let startupRegistrationTested = false;

async function settle(cdp) {
  await cdp.evaluate(`(async () => {
    await document.fonts.ready;
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    await Promise.all(document.getAnimations().filter(animation => animation.effect?.getComputedTiming().iterations !== Infinity)
      .map(animation => animation.finished.catch(() => {})));
  })()`);
}

async function trayAction(child, action, screenshot) {
  const powershell = path.join(process.env.SystemRoot, "System32/WindowsPowerShell/v1.0/powershell.exe");
  const args = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File",
    path.resolve("tool/windows_tray_smoke.ps1"), "-ApplicationPid", String(child.pid), "-Action", action];
  if (screenshot) args.push("-IconScreenshot", screenshot);
  const shell = spawn(powershell, args, { windowsHide: true, timeout: 25000, stdio: ["ignore", "pipe", "pipe"] });
  const output = [];
  const errors = [];
  shell.stdout.on("data", chunk => output.push(chunk));
  shell.stderr.on("data", chunk => errors.push(chunk));
  const [code] = await once(shell, "exit");
  assert.equal(code, 0, Buffer.concat(errors).toString());
  return JSON.parse(Buffer.concat(output).toString().trim().replace(/^\uFEFF/, ""));
}

async function port() {
  const server = net.createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert(address && typeof address === "object");
  await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  return address.port;
}

async function waitFor(check, label, milliseconds = 20000) {
  const deadline = Date.now() + milliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await check();
      if (result) return result;
    } catch (error) { lastError = error; }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out: ${label}`, { cause: lastError });
}

async function connectCdp(debugPort) {
  const target = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    const targets = await response.json();
    return targets.find((target) => target.type === "page" && target.url.includes("tauri.localhost"));
  }, "WebView target");
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let nextId = 1;
  const pending = new Map();
  const exceptions = [];
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.method === "Runtime.exceptionThrown") exceptions.push(message.params);
    const request = pending.get(message.id);
    if (!request) return;
    clearTimeout(request.timer);
    pending.delete(message.id);
    if (message.error) request.reject(new Error(JSON.stringify(message.error)));
    else request.resolve(message.result);
  });
  socket.addEventListener("close", () => {
    for (const request of pending.values()) {
      clearTimeout(request.timer);
      request.reject(new Error("WebView debugging connection closed"));
    }
    pending.clear();
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = nextId++;
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`CDP timeout: ${method}`));
    }, 15000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  await send("Runtime.enable");
  const evaluate = async (expression) => {
    const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  return { socket, send, evaluate, exceptions };
}

async function closeWindow(child) {
  assert(Number.isInteger(child.pid));
  const command = `
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class ShuTauriClose { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr handle, uint message, IntPtr wparam, IntPtr lparam); }'
$target = Get-Process -Id ${child.pid} -ErrorAction Stop
if ($target.MainWindowHandle -eq 0) { throw 'Tauri main window is missing' }
$running = [System.Threading.Mutex]::OpenExisting('ShuVPN.Desktop.Running')
$running.Dispose()
if (-not [ShuTauriClose]::PostMessage($target.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'WM_CLOSE failed' }
`;
  assert(process.env.SystemRoot, "SystemRoot is missing");
  const powershell = path.join(process.env.SystemRoot, "System32/WindowsPowerShell/v1.0/powershell.exe");
  const shell = spawn(powershell, ["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", command], {
    windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  shell.stderr.on("data", (chunk) => errors.push(chunk));
  const [code] = await once(shell, "exit");
  assert.equal(code, 0, Buffer.concat(errors).toString());
}

async function launch(index, scenario) {
  const debugPort = await port();
  const output = [];
  const child = spawn(executable, [...(index === 2 ? ["--hidden"] : []), "--data-dir", profile], {
    windowsHide: true,
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${debugPort}` },
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.on("data", (chunk) => output.push(chunk));
  child.stderr.on("data", (chunk) => output.push(chunk));
  const exit = once(child, "exit");
  const run = { index, pid: child.pid, passed: false, startedAt: new Date().toISOString() };
  child.on("exit", (code, signal) => {
    run.exitCode = code;
    run.exitSignal = signal;
    run.exitedAt = new Date().toISOString();
  });
  runs.push(run);
  let cdp;
  try {
    cdp = await connectCdp(debugPort);
    await waitFor(() => cdp.evaluate(`document.querySelector('#connection-state')?.textContent === '尚未连接'`), "native startup");
    if (index === 2) {
      assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible',{label:'main'})`), false, "autostart mode must start hidden");
      await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|show',{label:'main'})`);
    }
    assert.equal(await cdp.evaluate(`document.querySelector('#connect-label').textContent`), "登录账户");
    assert.equal(await cdp.evaluate(`document.querySelector('.app-footer summary').textContent`), `${expectedVersion} · 开源许可`);
    await settle(cdp);
    const home = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path.join(report, `home-${index}.png`), Buffer.from(home.data, "base64"));
    await scenario(cdp, child);
    await settle(cdp);
    const image = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path.join(report, `window-${index}.png`), Buffer.from(image.data, "base64"));
    assert.deepEqual(cdp.exceptions, [], "WebView threw an unhandled exception");
    await cdp.evaluate(`document.querySelector('#close-window').click()`);
    await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', {label:'main'}).then(value => !value)`), "titlebar close hides window");
    assert.equal(child.exitCode, null, "closing must keep the process alive");
    assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('desktop_snapshot').then(value => value.connection.phase)`), "disconnected");
    const secondary = spawn(executable, ["--data-dir", profile], { windowsHide: true, stdio: "ignore" });
    const [secondaryCode] = await once(secondary, "exit");
    assert.equal(secondaryCode, 0, "a second launch must restore the existing window");
    await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', {label:'main'})`), "launcher restores hidden window");
    await closeWindow(child);
    await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible', {label:'main'}).then(value => !value)`), "WM_CLOSE hides window");
    assert.equal(child.exitCode, null);
    await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|show', {label:'main'})`);
    // Exit is a separate action which must drain native work and terminate.
    if (nativeTray) await trayAction(child, "exit");
    else await cdp.evaluate(`setTimeout(() => window.__TAURI_INTERNALS__.invoke('exit_application'), 50); true`);
    const [code, signal] = await Promise.race([
      exit,
      new Promise((_, reject) => {
        const timer = setTimeout(() => reject(new Error("Tauri did not exit after explicit exit")), 15000);
        timer.unref();
      }),
    ]);
    assert.equal(code, 0);
    assert.equal(signal, null);
    run.passed = true;
    run.exitCode = code;
  } catch (error) {
    run.error = String(error);
    if (cdp) {
      run.exceptions = cdp.exceptions;
      try {
        const image = await cdp.send("Page.captureScreenshot", { format: "png" });
        await writeFile(path.join(report, `failure-${index}.png`), Buffer.from(image.data, "base64"));
        const document = await cdp.evaluate(`({title:document.title,text:document.body.innerText,profile:document.querySelector('#profile')?.value})`);
        await writeFile(path.join(report, `failure-${index}.json`), JSON.stringify(document, null, 2));
      } catch (diagnosticError) {
        run.diagnosticError = String(diagnosticError);
      }
    }
    throw error;
  } finally {
    cdp?.socket.close();
    if (child.exitCode === null && child.signalCode === null) {
      child.kill();
      await exit;
      run.forcedCleanup = true;
    }
    await writeFile(path.join(report, `process-${index}.log`), Buffer.concat(output));
  }
}

const navigate = async (cdp, page, heading) => {
  await cdp.evaluate(`document.querySelector('[data-page=${JSON.stringify(page)}]').click()`);
  await waitFor(() => cdp.evaluate(`document.querySelector('h1')?.textContent === ${JSON.stringify(heading)}`), heading);
};

async function holdNativeResponses(cdp) {
  // Tauri's invoke function is read-only. Its exposed debugging callback Map
  // lets this driver delay actual replies without replacing their contents.
  await cdp.evaluate(`(() => {
    const callbacks = window.__TAURI_INTERNALS__.callbacks;
    const original = callbacks.set;
    window.__heldReplies = {};
    window.__restoreReplies = () => { callbacks.set = original; };
    callbacks.set = function (id, callback) {
      const key = window.__holdNextReply;
      if (key) {
        window.__holdNextReply = null;
        return original.call(this, id, value => {
          window.__heldReplies[key] = () => callback(value);
        });
      }
      return original.call(this, id, callback);
    };
  })()`);
}

try {
  await launch(1, async (cdp, child) => {
    await cdp.evaluate("document.fonts.ready.then(() => true)");
    assert.equal(await cdp.evaluate(`document.fonts.check('14px "LXGW WenKai GB"') && [...document.fonts].some(face => face.family.includes('LXGW') && face.status === 'loaded')`), true, "bundled WenKai GB must load");
    const layout = await cdp.evaluate(`(() => {
      const main = document.querySelector('main');
      return {width:innerWidth,height:innerHeight,scrollHeight:main.scrollHeight,clientHeight:main.clientHeight,scrollWidth:document.documentElement.scrollWidth};
    })()`);
    assert(layout.scrollHeight <= layout.clientHeight + 1 && layout.scrollWidth <= layout.width, `home must fit the first viewport: ${JSON.stringify(layout)}`);
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: 380, height: 540, deviceScaleFactor: 2, mobile: false });
    await settle(cdp);
    const compact = await cdp.evaluate(`(() => { const main=document.querySelector('main'); return {height:main.clientHeight,scroll:main.scrollHeight,width:document.documentElement.scrollWidth}; })()`);
    assert(compact.scroll <= compact.height + 1 && compact.width <= 380, `minimum viewport must fit: ${JSON.stringify(compact)}`);
    await cdp.evaluate(`document.querySelector('#feedback').hidden=false; document.querySelector('#feedback').textContent='正在处理…';
      document.querySelector('#failure').hidden=false; document.querySelector('#failure').textContent='本地验证错误提示\\n'.repeat(10)`);
    const withStatus = await cdp.evaluate(`(() => { const main=document.querySelector('main'); return {height:main.clientHeight,scroll:main.scrollHeight}; })()`);
    assert(withStatus.scroll <= withStatus.height + 1, `pending actions and errors must preserve the first viewport: ${JSON.stringify(withStatus)}`);
    await cdp.evaluate(`document.querySelector('#feedback').hidden=true; document.querySelector('#failure').hidden=true`);
    const smallImage = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path.join(report, "home-minimum.png"), Buffer.from(smallImage.data, "base64"));
    await cdp.send("Emulation.clearDeviceMetricsOverride");
    await cdp.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] });
    assert.equal(await cdp.evaluate(`getComputedStyle(document.querySelector('#content')).animationName`), "none");
    await cdp.send("Emulation.setEmulatedMedia", { features: [] });
    if (nativeTray) {
      const tray = await trayAction(child, "inspect", path.join(report, "tray-disconnected.png"));
      assert(tray.items.some(item => item.text === "尚未连接" && !item.enabled));
      assert(tray.items.some(item => item.text === "连接校园网络" && item.enabled));
      assert(tray.items.some(item => item.text === "断开连接并清理" && !item.enabled));
      await writeFile(path.join(report, "native-tray.json"), JSON.stringify(tray, null, 2));
      await cdp.evaluate(`document.querySelector('#minimize-window').click()`);
      await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_minimized',{label:'main'})`), "titlebar minimize");
      await trayAction(child, "show");
      await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_minimized',{label:'main'}).then(value => !value)`), "tray restores minimized window");
      await trayAction(child, "hide");
      await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|is_visible',{label:'main'}).then(value => !value)`), "tray hides window");
      await trayAction(child, "settings");
      await waitFor(() => cdp.evaluate(`document.querySelector('h1')?.textContent === '网络设置'`), "tray settings navigation");
      const httpBefore = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('desktop_snapshot').then(value => value.settings.httpProxyEnabled)`);
      await trayAction(child, "http");
      await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('desktop_snapshot').then(value => value.settings.httpProxyEnabled !== ${httpBefore})`), "tray proxy toggle reaches core");
      await trayAction(child, "http");
      await waitFor(() => cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('desktop_snapshot').then(value => value.settings.httpProxyEnabled === ${httpBefore})`), "tray proxy toggle restores setting");
      await navigate(cdp, "connect", "尚未连接");
    }
    await cdp.evaluate(`document.querySelector('#connect').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('h1')?.textContent === '校园账户'`), "unsigned connection action");
    assert.equal(await cdp.evaluate(`document.querySelector('#feedback').hidden`), true, "opening login must not report a completed operation");
    assert.equal(await cdp.evaluate(`document.querySelector('#account-state').textContent`), "尚未登录");
    assert.equal(await cdp.evaluate(`document.querySelector('#password').type`), "password");
    await navigate(cdp, "settings", "网络设置");
    await cdp.evaluate(`document.querySelector('button[data-palette=jade]').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '配色已保存'`), "preset save");
    assert.equal(await cdp.evaluate(`document.documentElement.dataset.palette`), "jade");
    await cdp.evaluate(`document.querySelector('.theme-editor').open = true;
      document.querySelector('#theme-name').value='测试主题';
      const accent=document.querySelector('[data-mode=light][data-color=accent]');
      accent.value='#8a456c'; accent.dispatchEvent(new Event('input', {bubbles:true}));
      document.querySelector('#theme-form').requestSubmit()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '配色已保存'`), "custom theme save");
    assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('appearance_current').then(value => value.customTheme.light.accent)`), "#8a456c");
    const exports = path.join(report, "theme-exports");
    await mkdir(exports);
    await cdp.send("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: exports });
    await cdp.send("Runtime.evaluate", {
      expression: `document.querySelector('#theme-export').click()`, userGesture: true,
    });
    const exported = await waitFor(async () => {
      const content = await readFile(path.join(exports, "shuvpn-theme.json"), "utf8");
      return JSON.parse(content);
    }, "native theme download");
    assert.equal(exported.name, "测试主题");
    assert.equal(exported.light.accent, "#8a456c");
    const imported = path.join(report, "import-theme.json");
    await writeFile(imported, JSON.stringify({ ...exported, name: "导入主题" }));
    const document = await cdp.send("DOM.getDocument");
    const fileInput = await cdp.send("DOM.querySelector", { nodeId: document.root.nodeId, selector: "#theme-import" });
    await cdp.send("DOM.setFileInputFiles", { nodeId: fileInput.nodeId, files: [imported] });
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '正在预览，尚未保存'`), "theme file import preview");
    assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('appearance_current').then(value => value.customTheme.name)`), "测试主题", "import must not silently persist a draft");
    await cdp.evaluate(`document.querySelector('#theme-form').requestSubmit()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '配色已保存'`), "imported theme save");
    assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('appearance_current').then(value => value.customTheme.name)`), "导入主题");
    const delayedImport = path.join(report, "delayed-theme.json");
    await writeFile(delayedImport, JSON.stringify({ ...exported, name: "迟到的导入" }));
    await cdp.evaluate(`window.__originalThemeRead=File.prototype.text;
      File.prototype.text=function() { const file=this; return new Promise(resolve => {
        window.__releaseThemeRead=async () => resolve(await window.__originalThemeRead.call(file));
      }); }`);
    await cdp.send("DOM.setFileInputFiles", { nodeId: fileInput.nodeId, files: [delayedImport] });
    await waitFor(() => cdp.evaluate(`typeof window.__releaseThemeRead === 'function'`), "theme file read held");
    await cdp.evaluate(`document.querySelector('#theme-name').value='保留手动编辑';
      document.querySelector('#theme-name').dispatchEvent(new Event('input', {bubbles:true}));
      File.prototype.text=window.__originalThemeRead; window.__releaseThemeRead()`);
    await settle(cdp);
    assert.equal(await cdp.evaluate(`document.querySelector('#theme-name').value`), "保留手动编辑", "a late file read must not replace later manual edits");
    await cdp.evaluate(`document.querySelector('button[data-palette=iris]').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '配色已保存' && document.documentElement.dataset.palette === 'iris'`), "second preset save");
    await holdNativeResponses(cdp);
    await cdp.evaluate(`window.__holdNextReply='appearance'; document.querySelector('button[data-palette=amber]').click()`);
    await waitFor(() => cdp.evaluate(`typeof window.__heldReplies.appearance === 'function'`), "native theme reply held");
    await navigate(cdp, "records", "连接记录");
    await navigate(cdp, "settings", "网络设置");
    await cdp.evaluate(`document.querySelector('.theme-editor').open=true; document.querySelector('#theme-name').value='新的预览';
      for (const field of document.querySelectorAll('[data-color=accent]')) field.value='#416b51';
      document.querySelector('#theme-name').dispatchEvent(new Event('input', {bubbles:true}))`);
    await cdp.evaluate(`window.__heldReplies.appearance(); window.__restoreReplies()`);
    assert.equal(await cdp.evaluate(`document.documentElement.style.getPropertyValue('--accent')`), "#416b51", "a late save must preserve a newer page's preview");
    await holdNativeResponses(cdp);
    await cdp.evaluate(`window.__holdNextReply='same-page-appearance'; document.querySelector('button[data-palette=jade]').click()`);
    await waitFor(() => cdp.evaluate(`typeof window.__heldReplies['same-page-appearance'] === 'function'`), "same-page save reply held");
    await cdp.evaluate(`document.querySelector('#theme-name').value='后续编辑';
      for (const field of document.querySelectorAll('[data-color=accent]')) field.value='#525da2';
      document.querySelector('#theme-name').dispatchEvent(new Event('input', {bubbles:true}));
      window.__heldReplies['same-page-appearance'](); window.__restoreReplies()`);
    await settle(cdp);
    assert.equal(await cdp.evaluate(`document.querySelector('#theme-status').textContent.includes('未保存')`), true, "a reply must not mark newer edits as saved");
    assert.equal(await cdp.evaluate(`document.querySelector('button[data-palette=custom]').getAttribute('aria-pressed')`), "true");
    await cdp.evaluate(`document.querySelector('#theme-name').value='   ';
      document.querySelector('#theme-name').dispatchEvent(new Event('input', {bubbles:true}));
      document.querySelector('#theme-export').click();
      document.querySelector('button[data-palette=iris]').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#theme-status').textContent === '配色已保存' && document.documentElement.dataset.palette === 'iris'`), "a preset must recover from an invalid custom draft");
    await waitFor(() => cdp.evaluate(`!document.querySelector('#autostart').disabled`), "startup setting available");
    const startupBefore = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('startup_enabled')`);
    const registry = spawn(path.join(process.env.SystemRoot, "System32/reg.exe"),
      ["query", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "ShuVPN"],
      { windowsHide: true, stdio: "ignore" });
    const [registrationCode] = await once(registry, "exit");
    assert([0, 1].includes(registrationCode), "startup registry query failed");
    if (!startupBefore && registrationCode === 1) {
      try {
        await cdp.evaluate(`document.querySelector('#autostart').click()`);
        await waitFor(() => cdp.evaluate(`document.querySelector('#startup-status').textContent.startsWith('已启用')`), "native startup registration");
        assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('startup_enabled')`), true);
      } finally {
        await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('startup_set', {enabled:false})`);
      }
      assert.equal(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('startup_enabled')`), false);
      startupRegistrationTested = true;
    }
    await cdp.evaluate(`document.querySelector('[name=httpPort]').value='18443'; document.querySelector('[name=themeMode]').value='light'; document.querySelector('#settings-form').requestSubmit()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#saved').textContent.startsWith('已保存')`), "settings save");
    await navigate(cdp, "connect", "尚未连接");
    await settle(cdp);
    const light = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path.join(report, "home-light.png"), Buffer.from(light.data, "base64"));
    assert.equal(await cdp.evaluate(`document.documentElement.scrollWidth <= innerWidth`), true);
    await navigate(cdp, "settings", "网络设置");
    await cdp.evaluate(`(() => { document.querySelector('.theme-editor').open=true; const accent=document.querySelector('[data-mode=light][data-color=accent]');
      accent.value='#ffee00'; accent.dispatchEvent(new Event('input', {bubbles:true})); })()`);
    await settle(cdp);
    const buttonColors = await cdp.evaluate(`(() => {
      const style=getComputedStyle(document.querySelector('#settings-form .primary'));
      return [style.color,style.backgroundColor];
    })()`);
    const luminance = color => {
      const channels = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(value => {
        const channel = value / 255;
        return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
      });
      return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
    };
    const foreground = luminance(buttonColors[0]);
    const background = luminance(buttonColors[1]);
    const contrast = (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
    assert(contrast >= 4.5, `custom theme button contrast must remain readable: ${contrast}`);
    await writeFile(path.join(report, "theme-contrast.json"), JSON.stringify({
      foreground: buttonColors[0], background: buttonColors[1], ratio: contrast, minimum: 4.5,
    }, null, 2));
    await cdp.evaluate(`document.querySelector('#theme-reset').click()`);
    await cdp.evaluate(`document.querySelector('[name=themeMode]').value='dark'; document.querySelector('#settings-form').requestSubmit()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#saved').textContent.startsWith('已保存')`), "dark setting");
    await navigate(cdp, "connect", "尚未连接");
    await settle(cdp);
    const dark = await cdp.send("Page.captureScreenshot", { format: "png" });
    await writeFile(path.join(report, "home-dark.png"), Buffer.from(dark.data, "base64"));
    await navigate(cdp, "routing", "规则分流");
    await waitFor(() => cdp.evaluate(`document.querySelector('#profile')?.value.length > 0`), "routing document");
    await holdNativeResponses(cdp);
    await cdp.evaluate(`(async () => {
      window.__holdNextReply = 'campus-only';
      document.querySelector('#campus-only').click();
      await Promise.resolve();
    })()`);
    await cdp.evaluate(`(async () => {
      window.__holdNextReply = 'campus-direct';
      document.querySelector('#campus-direct').click();
      await Promise.resolve();
    })()`);
    await waitFor(() => cdp.evaluate(`Object.keys(window.__heldReplies).length === 2`), "both native templates");
    await cdp.evaluate(`window.__heldReplies['campus-only']()`);
    await cdp.evaluate(`window.__heldReplies['campus-direct'](); window.__restoreReplies()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#profile').value.includes('MATCH,DIRECT')`), "template");
    await cdp.evaluate(`document.querySelector('#prepare').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#commit').disabled === false`), "routing preview");
    await cdp.evaluate(`document.querySelector('#commit').click()`);
    await waitFor(() => cdp.evaluate(`document.querySelector('#preview').textContent.startsWith('已应用版本')`), "routing commit");
    await cdp.evaluate(`document.querySelector('#prepare').click()`);
    await waitFor(() => cdp.evaluate(`!document.querySelector('#commit').disabled`), "navigation preview");
    await holdNativeResponses(cdp);
    await cdp.evaluate(`(() => {
      window.__holdNextReply = 'discard';
      document.querySelector('[data-page=settings]').click();
    })()`);
    await waitFor(() => cdp.evaluate(`typeof window.__heldReplies.discard === 'function'`), "native discard response held");
    await navigate(cdp, "connect", "尚未连接");
    await cdp.evaluate(`window.__heldReplies.discard(); window.__restoreReplies()`);
    assert.equal(await cdp.evaluate(`document.querySelector('#connection-state')?.textContent`), "尚未连接");
    await navigate(cdp, "routing", "规则分流");
    await waitFor(() => cdp.evaluate(`document.querySelector('#profile')?.value.includes('MATCH,DIRECT')`), "routing after competing navigation");
    await cdp.evaluate(`document.querySelector('#profile').value='invalid: ['; document.querySelector('#profile').dispatchEvent(new Event('input', {bubbles:true})); document.querySelector('#prepare').click()`);
    await waitFor(() => cdp.evaluate(`!document.querySelector('#failure').hidden`), "invalid routing error");
    assert.equal(await cdp.evaluate(`document.querySelector('#commit').disabled`), true);
  });
  await launch(2, async (cdp) => {
    await navigate(cdp, "settings", "网络设置");
    assert.equal(await cdp.evaluate(`document.querySelector('[name=httpPort]').value`), "18443");
    assert.equal(await cdp.evaluate(`document.querySelector('[name=themeMode]').value`), "dark");
    assert.equal(await cdp.evaluate(`document.documentElement.dataset.palette`), "iris");
    assert.equal(await cdp.evaluate(`document.querySelector('[data-mode=light][data-color=accent]').value`), "#8a456c");
    await navigate(cdp, "routing", "规则分流");
    await waitFor(() => cdp.evaluate(`document.querySelector('#profile')?.value.includes('MATCH,DIRECT')`), "persisted routing");
    await navigate(cdp, "records", "连接记录");
    await waitFor(() => cdp.evaluate(`document.querySelector('#records').textContent === '当前会话暂无连接记录'`), "empty records");
  });
  assert.equal(createHash("sha256").update(await readFile(executable)).digest("hex"), productHash);
  await writeFile(path.join(report, "verification.json"), JSON.stringify({
    passed: true, executable, sha256: productHash, runs, realCampusLoginTested: false, nativeTrayTested: nativeTray, startupRegistrationTested,
    scope: "Windows WebView2 native IPC, default/minimum viewport including pending/error feedback, WenKai GB, reduced motion, theme import/export/persistence/late replies/contrast, hidden launch, titlebar/WM_CLOSE hide, single-instance restore, graceful exit"
      + (startupRegistrationTested ? ", startup register/restore" : ", existing startup registration preserved")
      + (nativeTray ? ", real Explorer tray menu/state/minimize/restore/hide/navigation/proxy toggle/exit" : "") + "; no VPN connection",
  }, null, 2));
  console.log("TAURI_WINDOWS_OFFLINE_WEBVIEW_PERSISTENCE_AND_CLOSE_PASSED");
} catch (error) {
  await writeFile(path.join(report, "verification.json"), JSON.stringify({
    passed: false, executable, sha256: productHash, runs, error: String(error), realCampusLoginTested: false,
  }, null, 2));
  throw error;
}
