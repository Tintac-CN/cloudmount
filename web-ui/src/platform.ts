/**
 * 平台适配层 —— 服务端/边缘网关专用（浏览器 → REST /api/*）。
 *
 * 桌面端已切换为纯 Rust（egui + wgpu + rfd，apps/desktop），
 * 不再使用本 web-ui。本模块保留 fetch 单路径。
 */

/** 当前宿主类型（浏览器远程管理）。 */
export function hostKind(): "web" {
  return "web";
}

/**
 * 后端调用入口：`POST /api/{cmd}`（cloudmountd --serve 提供）。
 */
export async function call<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
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
