import { hostKind } from "./platform";
import { getStatus, type StatusInfo } from "./api";

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (text) e.textContent = text;
  return e;
}

async function render(): Promise<void> {
  const root = document.getElementById("app")!;
  root.replaceChildren();

  root.append(el("h1", "CloudMount 云挂"));

  const host = el("p");
  host.textContent =
    hostKind() === "desktop"
      ? "桌面模式（Tauri 壳 · 本机挂载）"
      : "边缘网关模式（浏览器远程管理）";
  root.append(host);

  try {
    const s: StatusInfo = await getStatus();
    const list = el("ul");
    list.append(
      el("li", `版本：${s.version}`),
      el("li", `登录：${s.loggedIn ? "已登录" : "未登录"}（通道：${s.channel}）`),
      el("li", `同步：${s.syncing ? "进行中" : "空闲"}`),
      el("li", `队列：↑${s.pendingUploads} ↓${s.pendingDownloads}`),
      el(
        "li",
        `缓存：${(s.cacheBytes / 1e9).toFixed(2)} / ${(s.cacheLimitBytes / 1e9).toFixed(0)} GB`,
      ),
    );
    root.append(el("h2", "状态"), list);
  } catch (err) {
    root.append(el("p", `后端未就绪：${String(err)}`));
  }
}

render();
