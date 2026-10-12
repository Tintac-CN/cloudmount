/**
 * 平台适配层 —— CloudMount "一套前端、两种宿主" 的关键。
 *
 * - **Tauri 桌面壳**：`window.__TAURI_INTERNALS__` 存在，走 `invoke()` 调
 *   Rust 命令（同进程 IPC，最低延迟）。
 * - **边缘网关 Web**：纯浏览器环境，走 REST `/api/*`（cloudmountd 提供）。
 *
 * UI 代码只 import 本模块的 `call`，永远不直接接触 Tauri API 或 fetch，
 * 从而保证两端 100% 复用。
 */

/** 当前是否运行在 Tauri 桌面壳内（依赖 tauri.conf.json 的 withGlobalTauri）。 */
export function isTauri(): boolean {
  return (
    typeof window !== "undefined" &&
    (window as unknown as { __TAURI__?: unknown }).__TAURI__ !== undefined
  );
}

/** 当前宿主类型（用于 UI 显示与埋点）。 */
export function hostKind(): "desktop" | "web" {
  return isTauri() ? "desktop" : "web";
}

/**
 * 统一的后端调用入口。
 *
 * 桌面端经 `window.__TAURI__`（withGlobalTauri 注入，前端零 npm 依赖）调用
 * Tauri 命令 `cmd`；Web 端映射到 `POST /api/{cmd}`。
 * 两端的后端（Tauri Rust / cloudmountd）必须暴露同名命令。
 */
export async function call<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    const tauri = (window as unknown as {
      __TAURI__: { core: { invoke: (c: string, a?: unknown) => Promise<T> } };
    }).__TAURI__;
    return tauri.core.invoke(cmd, args);
  }
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  if (!res.ok) {
    throw new Error(`API ${cmd} failed: HTTP ${res.status}`);
  }
  return (await res.json()) as T;
}
