import { backend } from "./api";
import { element } from "./dom";

function isFields(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fields(value: unknown): Record<string, unknown> {
  return isFields(value) ? value : {};
}

function label(value: unknown, fallback = "未提供"): string {
  return typeof value === "string" || typeof value === "number" ? String(value) : fallback;
}

function actionLabel(value: unknown): string {
  if (value === "DIRECT") return "直连";
  if (value === "SHU-VPN") return "校园隧道";
  if (value === "REJECT") return "拒绝";
  return "未成功出站";
}

function recordCard(record: unknown): HTMLElement {
  const entry = fields(record);
  const flow = fields(entry.flow);
  const decision = fields(entry.decision);
  const card = document.createElement("article");
  card.className = "record-card";
  const summary = document.createElement("header");
  const target = document.createElement("strong");
  target.textContent = `${label(flow.host)}:${label(flow.port, "—")}`;
  const action = document.createElement("span");
  action.className = entry.error ? "record-action failed" : "record-action";
  action.textContent = actionLabel(entry.outbound ?? (entry.error === "rejected" ? "REJECT" : null));
  summary.append(target, action);
  const description = document.createElement("p");
  const succeeded = entry.outbound === "DIRECT" || entry.outbound === "SHU-VPN";
  description.textContent = entry.message ? label(entry.message)
    : `${label(flow.network).toUpperCase()} · ${label(flow.inbound).toUpperCase()} · ${succeeded ? "成功建立出站" : "尚无成功出站结果"}`;
  const details = document.createElement("details");
  const caption = document.createElement("summary");
  caption.textContent = "规则详情";
  const evidence = document.createElement("p");
  evidence.textContent = entry.decision
    ? `版本 ${label(decision.version)} · ${label(decision.rule_id)} · ${label(decision.source)}\n规则动作：${actionLabel(decision.action)}\n${label(decision.reason, "")}`
    : `规则判定前失败 · 版本 ${label(entry.version)}`;
  details.append(caption, evidence);
  card.append(summary, description, details);
  return card;
}

export async function showRecords(root: HTMLElement, run: (action: () => Promise<void>) => void): Promise<void> {
  root.innerHTML = `<header><h1>连接记录</h1><p>最近最多 128 条出站结果，成功出站不代表请求已完成。</p></header>
    <div class="actions"><button id="refresh-records">刷新记录</button></div><section id="records" aria-live="polite">正在读取…</section>`;
  const load = async () => {
    const entries = await backend.records();
    if (!root.isConnected) return;
    const list = element(root, "#records", HTMLElement);
    if (!entries.length) list.textContent = "当前会话暂无连接记录";
    else list.replaceChildren(...entries.toReversed().map(recordCard));
  };
  element(root, "#refresh-records", HTMLButtonElement).onclick = () => run(load);
  await load();
}
