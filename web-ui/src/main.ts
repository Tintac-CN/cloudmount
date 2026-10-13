import { hostKind } from "./platform";
import {
  clearCache,
  exportLogs,
  getCacheStats,
  getQuota,
  getStatus,
  setSyncRoot,
  togglePause,
  type CacheInfo,
  type QuotaInfo,
  type StatusInfo,
} from "./api";

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (text) e.textContent = text;
  return e;
}

/** 字节数人性化。 */
function humanBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} ${units[i]}`;
}

/** 百分比条（缓存占用/配额用）。 */
function bar(ratio: number, label: string): HTMLElement {
  const wrap = el("div", "");
  wrap.className = "bar";
  const fill = el("div", "");
  fill.className = "bar-fill";
  fill.style.width = `${Math.min(100, Math.max(0, ratio * 100)).toFixed(1)}%`;
  wrap.append(fill);
  const text = el("span", label);
  text.className = "bar-label";
  wrap.append(text);
  return wrap;
}

/** 键值行。 */
function row(k: string, v: string): HTMLLIElement {
  const li = el("li", "");
  li.append(el("span", k), el("b", v));
  return li;
}

type Tab = "status" | "cache" | "settings";

// ============ 状态页 ============

async function renderStatus(root: HTMLElement): Promise<void> {
  const box = el("section", "");
  try {
    const s: StatusInfo = await getStatus();
    const list = el("ul", "");
    list.append(
      row("版本", s.version),
      row("登录", s.loggedIn ? "已登录" : "未登录"),
      row(
        "同步",
        s.paused ? "已暂停" : s.syncing ? "进行中" : "空闲",
      ),
      row("累计上传", String(s.totalUploaded ?? 0)),
      row("累计下载", String(s.totalDownloads ?? 0)),
      row("冲突", String(s.conflicts)),
      row("错误", String(s.errors)),
    );
    box.append(el("h2", "状态"), list);
    if (s.lastMessage) {
      const msg = el("p", s.lastMessage);
      msg.className = "muted";
      box.append(msg);
    }
    // 配额（引擎可用时）
    try {
      const q: QuotaInfo = await getQuota();
      if (q.total > 0) {
        box.append(
          el("h2", "云端配额"),
          bar(q.used / q.total, `${humanBytes(q.used)} / ${humanBytes(q.total)}`),
        );
      }
    } catch {
      // 引擎未运行时无配额——静默
    }
  } catch (err) {
    box.append(el("p", `后端未就绪：${String(err)}`));
  }
  root.append(box);
}

// ============ 缓存页 ============

async function renderCache(root: HTMLElement): Promise<void> {
  const box = el("section", "");
  try {
    const c: CacheInfo = await getCacheStats();
    const total = c.hits + c.misses;
    const hitRate = total === 0 ? 0 : c.hits / total;
    const list = el("ul", "");
    list.append(
      row("占用", `${humanBytes(c.bytes)} / ${humanBytes(c.limitBytes)}`),
      row("命中率", `${(hitRate * 100).toFixed(1)}%（${c.hits}/${total}）`),
      row("写入", String(c.inserts)),
      row("淘汰", String(c.evictions)),
    );
    box.append(el("h2", "内容缓存"), list);
    box.append(
      bar(c.bytes / Math.max(1, c.limitBytes), `${humanBytes(c.bytes)}`),
    );
    const btn = el("button", "一键清理");
    btn.onclick = async () => {
      btn.disabled = true;
      btn.textContent = "清理中…";
      try {
        await clearCache();
        btn.textContent = "已清理 ✓";
        render();
      } catch (e) {
        btn.textContent = `清理失败: ${String(e)}`;
      }
      setTimeout(() => {
        btn.disabled = false;
        btn.textContent = "一键清理";
      }, 2000);
    };
    box.append(btn);
  } catch (err) {
    box.append(
      el("h2", "内容缓存"),
      el("p", `缓存不可用（引擎未运行）：${String(err)}`),
    );
  }
  root.append(box);
}

// ============ 设置页 ============

async function renderSettings(root: HTMLElement): Promise<void> {
  const box = el("section", "");

  // 暂停/恢复
  const pauseBox = el("div", "");
  const pauseBtn = el("button", "查询中…");
  pauseBtn.onclick = async () => {
    try {
      const paused = await togglePause();
      pauseBtn.textContent = paused ? "恢复同步" : "暂停同步";
    } catch (e) {
      pauseBtn.textContent = `失败: ${String(e)}`;
    }
  };
  try {
    const s = await getStatus();
    pauseBtn.textContent = s.paused ? "恢复同步" : "暂停同步";
  } catch {
    pauseBtn.textContent = "暂停/恢复";
  }
  pauseBox.append(el("h2", "同步控制"), pauseBtn);
  box.append(pauseBox);

  // 同步根目录
  const rootBox = el("div", "");
  const input = el("input", "");
  input.placeholder = "D:\\CloudMount（同步根目录）";
  const apply = el("button", "应用");
  apply.onclick = async () => {
    try {
      await setSyncRoot(input.value.trim());
      apply.textContent = "已应用 ✓";
      setTimeout(() => (apply.textContent = "应用"), 1500);
    } catch (e) {
      apply.textContent = `失败: ${String(e)}`;
    }
  };
  rootBox.append(el("h2", "同步根目录"), input, apply);
  box.append(rootBox);

  // 日志导出
  const logBox = el("div", "");
  const exportBtn = el("button", "导出日志");
  exportBtn.onclick = async () => {
    try {
      const lines = await exportLogs();
      const blob = new Blob([lines.join("\n")], {
        type: "text/plain;charset=utf-8",
      });
      const a = el("a", "") as HTMLAnchorElement;
      a.href = URL.createObjectURL(blob);
      a.download = `cloudmount-log-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.txt`;
      a.click();
      URL.revokeObjectURL(a.href);
    } catch (e) {
      exportBtn.textContent = `导出失败: ${String(e)}`;
    }
  };
  logBox.append(el("h2", "日志"), exportBtn);
  const hint = el("p", "最近 2000 行（INFO 及以上）");
  hint.className = "muted";
  logBox.append(hint);
  box.append(logBox);

  root.append(box);
}

// ============ 主渲染 ============

async function render(tab: Tab = "status"): Promise<void> {
  const root = document.getElementById("app")!;
  root.replaceChildren();

  root.append(el("h1", "CloudMount 云挂"));
  const host = el("p", "");
  host.textContent =
    hostKind() === "desktop"
      ? "桌面模式（Tauri 壳 · 本机挂载）"
      : "边缘网关模式（浏览器远程管理）";
  host.className = "muted";
  root.append(host);

  // 标签导航
  const nav = el("nav", "");
  const tabs: [Tab, string][] = [
    ["status", "状态"],
    ["cache", "缓存"],
    ["settings", "设置"],
  ];
  for (const [key, label] of tabs) {
    const btn = el("button", label);
    if (key === tab) btn.className = "active";
    btn.onclick = () => render(key);
    nav.append(btn);
  }
  root.append(nav);

  switch (tab) {
    case "status":
      await renderStatus(root);
      break;
    case "cache":
      await renderCache(root);
      break;
    case "settings":
      await renderSettings(root);
      break;
  }
}

render();
