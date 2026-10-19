import { backend } from "./api";
import type { RoutingConfiguration } from "./api";
import { element, text } from "./dom";

export async function showRouting(root: HTMLElement, run: (action: () => Promise<void>) => void): Promise<() => Promise<void>> {
  const [initial, defaultProfile] = await Promise.all([
    backend.routing(), backend.template("campus-only"),
  ]);
  if (!root.isConnected) return () => Promise.resolve();
  root.innerHTML = `
    <header><h1>规则分流</h1><p>校验草稿后应用，已有连接保持原路径。</p></header>
    <section class="panel">
      <div class="actions"><strong id="version"></strong><button id="reload">重新读取</button><button id="rollback">回退上一版本</button></div>
      <div class="actions"><span>载入模板</span><button id="campus-only">仅校园</button><button id="campus-direct">校园优先</button></div>
      <label>配置档（YAML / JSON）<textarea id="profile" rows="8" spellcheck="false"></textarea></label>
      <details><summary>前置 / 后置规则与策略组覆写</summary><label>覆写（JSON）<textarea id="overrides" rows="8" spellcheck="false"></textarea></label></details>
      <div class="actions"><button id="prepare">校验并预览</button><button id="commit" class="primary" disabled>应用预览</button></div>
      <p id="preview" role="status">尚未生成预览</p>
    </section>`;
  const profile = element(root, "#profile", HTMLTextAreaElement);
  const overrides = element(root, "#overrides", HTMLTextAreaElement);
  const commit = element(root, "#commit", HTMLButtonElement);
  let current = initial;
  let revision = 0;
  let previewRevision: number | null = null;
  let previewId: string | null = null;
  const invalidate = async () => {
    const previous = previewId;
    previewId = null;
    previewRevision = null;
    revision++;
    commit.disabled = true;
    if (previous) await backend.discard(previous);
  };
  const adopt = (configuration: RoutingConfiguration, expectedRevision = revision) => {
    const unchanged = expectedRevision === revision;
    current = configuration;
    if (unchanged) {
      profile.value = current.profile_document || defaultProfile;
      overrides.value = current.overrides_document || "{}";
    }
    text(root, "#version", current.version === 0 ? "内置校园规则" : `生效版本 ${current.version}`);
    commit.disabled = true;
    previewRevision = null;
    revision++;
    return unchanged;
  };
  const discard = () => {
    // Invalidation takes effect before its first await, including keystrokes
    // received while a previous preview request is still in flight.
    const pending = invalidate();
    text(root, "#preview", "草稿已修改，请重新校验");
    run(async () => { await pending; });
  };
  adopt(initial);
  if (initial.version === 0) {
    text(root, "#preview", "当前使用内置校园规则。模板仅为草稿，校验并应用后才会生效。");
  }
  profile.addEventListener("input", discard);
  overrides.addEventListener("input", discard);
  element(root, "#reload", HTMLButtonElement).onclick = () => run(async () => {
    await invalidate();
    const requested = revision;
    const unchanged = adopt(await backend.routing(), requested);
    text(root, "#preview", unchanged ? "已读取生效配置" : "已读取生效版本，保留较新的未保存草稿");
  });
  for (const name of ["campus-only", "campus-direct"]) {
    element(root, `#${name}`, HTMLButtonElement).onclick = () => run(async () => {
      const clearing = invalidate();
      const requested = revision;
      await clearing;
      const template = await backend.template(name);
      if (requested !== revision || !root.isConnected) return;
      profile.value = template;
      discard();
    });
  }
  element(root, "#prepare", HTMLButtonElement).onclick = () => run(async () => {
    await invalidate();
    const requested = revision;
    commit.disabled = true;
    const preview = await backend.prepare(profile.value, overrides.value, current.version);
    if (requested !== revision || !root.isConnected) {
      await backend.discard(preview.previewId);
      return;
    }
    previewId = preview.previewId;
    previewRevision = requested;
    commit.disabled = false;
    text(root, "#preview", [`预览通过 · 将从版本 ${current.version} 应用`, ...preview.configuration.notices].join("\n"));
  });
  commit.onclick = () => run(async () => {
    if (previewRevision !== revision || !previewId) throw new Error("草稿已改变，请重新校验");
    const confirmed = previewId;
    const requested = revision;
    previewId = null;
    commit.disabled = true;
    const saved = await backend.commit(confirmed);
    const unchanged = adopt(saved, requested);
    text(root, "#preview", unchanged ? `已应用版本 ${saved.version}` : `已应用版本 ${saved.version}，较新的草稿已保留，请重新校验`);
  });
  element(root, "#rollback", HTMLButtonElement).onclick = () => run(async () => {
    const requested = revision;
    const saved = await backend.rollback(current.version);
    const unchanged = adopt(saved, requested);
    text(root, "#preview", unchanged ? `已回退，当前版本 ${saved.version}` : `已回退到版本 ${saved.version}，较新的草稿已保留`);
  });
  return invalidate;
}
