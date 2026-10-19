import "@picocss/pico/css/pico.css";
import "./style.css";
import thirdPartyNotices from "../THIRD_PARTY_NOTICES.md?raw";
import packageInfo from "../package.json";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { loadAppearance, currentAppearance } from "./theme";
import { backend, errorMessage } from "./api";
import type { DesktopSnapshot } from "./api";
import { applyTheme, element, text } from "./dom";
import { showSettings } from "./settings-page";
import { showRouting } from "./routing-page";
import { decorateIcons } from "./icons";
import { showRecords } from "./records-page";
import { createAccountPage } from "./account-page";
import { connectionPhaseLabels, createConnectionPage } from "./connection-page";

const app = element(document, "#app", HTMLDivElement);
app.innerHTML = `
  <a class="skip-link" href="#content">跳到内容</a>
  <header class="titlebar" data-tauri-drag-region>
    <span class="titlebar-brand" data-tauri-drag-region><span data-icon="shield"></span>ShuVPN <small data-tauri-drag-region>校园网络</small></span>
    <div class="titlebar-actions">
      <button id="theme-cycle" class="titlebar-btn" aria-label="切换日夜模式" title="切换日夜模式"><span data-icon="sun"></span></button>
    </div>
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

// Theme cycle: light → dark → system, persisted via saveSettings
const themeModes = ["light", "dark", "system"] as const;
const themeIcons: Record<string, string> = { light: "sun", dark: "moon", system: "monitor" };
const themeLabels: Record<string, string> = { light: "切换至深色", dark: "切换至跟随系统", system: "切换至浅色" };

function updateThemeCycleButton(mode: string): void {
  const btn = app.querySelector<HTMLButtonElement>("#theme-cycle");
  if (!btn) return;
  btn.setAttribute("aria-label", themeLabels[mode] ?? "切换主题");
  btn.setAttribute("title", themeLabels[mode] ?? "切换主题");
  btn.dataset.themeMode = mode;
  const span = btn.querySelector<HTMLElement>("[data-icon]");
  if (span) {
    span.dataset.icon = themeIcons[mode] ?? "sun";
    decorateIcons(btn);
  }
}

// Nailong connection sound: play 2s–5s of the laugh audio when connected
import nailongLaughUrl from "./assets/nailong/videoplayback.m4a?url";
let nailongAudioBuffer: AudioBuffer | null = null;
let nailongAudioCtx: AudioContext | null = null;

async function loadNailongSound(): Promise<void> {
  try {
    nailongAudioCtx = new AudioContext();
    const response = await fetch(nailongLaughUrl);
    const arrayBuffer = await response.arrayBuffer();
    nailongAudioBuffer = await nailongAudioCtx.decodeAudioData(arrayBuffer);
  } catch { /* audio unavailable */ }
}

let lastPhase = "disconnected";
function maybePlayNailongSound(phase: string): void {
  if (phase !== "connected" || lastPhase === "connected") { lastPhase = phase; return; }
  lastPhase = phase;
  if (currentAppearance()?.palette !== "nailong") return;
  if (!nailongAudioCtx || !nailongAudioBuffer) return;
  try {
    if (nailongAudioCtx.state === "suspended") void nailongAudioCtx.resume();
    const source = nailongAudioCtx.createBufferSource();
    source.buffer = nailongAudioBuffer;
    const gain = nailongAudioCtx.createGain();
    gain.gain.value = 0.85;
    source.connect(gain);
    gain.connect(nailongAudioCtx.destination);
    source.start(0, 2, 3); // 从第 2s 开始播放 3s，即 2s~5s
  } catch { /* ignore */ }
}

let currentPage = "connect";
let snapshot: DesktopSnapshot | null = null;
let pageRevision = 0;
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let disposePage: (() => Promise<void>) | undefined;
let feedbackTimer: ReturnType<typeof setTimeout> | undefined;
const pending = new Set<() => Promise<void>>();
const accountPage = createAccountPage(run, report);
const connectionPage = createConnectionPage({
  run, report, navigate,
  refreshView: () => { if (snapshot) update(snapshot); },
});

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

function navigate(page: string): void {
  clearTimeout(feedbackTimer);
  element(app, "#feedback", HTMLDivElement).hidden = true;
  element(app, "#failure", HTMLDivElement).hidden = true;
  void showPage(page).catch(report);
}

function update(current: DesktopSnapshot): void {
  snapshot = current;
  applyTheme(current.settings.themeMode);
  updateThemeCycleButton(current.settings.themeMode);
  maybePlayNailongSound(current.connection.phase);
  app.dataset.phase = current.connection.phase;
  text(app, "#app-state", connectionPhaseLabels[current.connection.phase]);
  const root = element(app, "#content", HTMLDivElement);
  if (currentPage === "connect") connectionPage.update(root, current);
  if (currentPage === "account") accountPage.update(root, current);
}

async function showPage(page: string): Promise<void> {
  if (!snapshot) return;
  const revision = ++pageRevision;
  const dispose = disposePage;
  disposePage = undefined;
  if (dispose) await dispose();
  if (revision !== pageRevision) return;
  if (currentPage === "account") await accountPage.cancelWork(snapshot);
  if (revision !== pageRevision) return;
  // 方向感切换：按导航栏顺序判断前进/后退，驱动页面滑入方向
  const order = ["connect", "account", "routing", "records", "settings"];
  const fromIndex = order.indexOf(currentPage);
  accountPage.dispose();
  currentPage = page;
  const toIndex = order.indexOf(page);
  const direction = toIndex >= fromIndex ? "forward" : "back";
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
  root.dataset.direction = direction;
  previous.replaceWith(root);
  element(app, "main", HTMLElement).scrollTop = 0;
  if (page === "settings") disposePage = showSettings(root, snapshot.settings, run);
  else if (page === "routing") {
    const dispose = await showRouting(root, run);
    if (revision !== pageRevision) await dispose();
    else disposePage = dispose;
  }
  else if (page === "account") {
    accountPage.show(root);
  }
  else if (page === "records") {
    await showRecords(root, run);
  } else {
    await connectionPage.show(root, snapshot);
  }
  if (revision === pageRevision && snapshot) {
    update(snapshot);
    root.focus({ preventScroll: true });
  }
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

// Theme cycle button: light → dark → system → light …
element(app, "#theme-cycle", HTMLButtonElement).onclick = () => {
  if (!snapshot) return;
  const current = snapshot.settings.themeMode;
  const idx = themeModes.indexOf(current as typeof themeModes[number]);
  const next = themeModes[(idx >= 0 ? idx + 1 : 0) % themeModes.length] ?? "system";
  run(async () => {
    await backend.saveSettings([{ key: "themeMode", value: next }]);
  });
};

window.addEventListener("pagehide", () => { clearTimeout(pollTimer); clearTimeout(feedbackTimer); });

async function start(): Promise<void> {
  await listen("shutdown-error", (event) => report(event.payload));
  await listen<string>("navigate", (event) => {
    if (["connect", "account", "routing", "records", "settings"].includes(event.payload)) navigate(event.payload);
  });
  await loadAppearance();
  void loadNailongSound(); // non-blocking, best-effort
  snapshot = await backend.snapshot();
  await showPage("connect");
  await poll();
}

void start().catch(report);
