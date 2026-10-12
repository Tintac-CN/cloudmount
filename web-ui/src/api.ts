/**
 * CloudMount API 面（前后端契约）。
 *
 * 桌面 Tauri 壳与边缘 cloudmountd 都要实现这些命令，命名与参数保持一致。
 * 当前为骨架；随里程碑逐步填充。
 */

import { call } from "./platform";

export interface StatusInfo {
  version: string;
  loggedIn: boolean;
  channel: "open" | "cookie";
  syncing: boolean;
  pendingUploads: number;
  pendingDownloads: number;
  cacheBytes: number;
  cacheLimitBytes: number;
}

/** 查询同步与账号状态（托盘/主界面共用）。 */
export function getStatus(): Promise<StatusInfo> {
  return call<StatusInfo>("get_status");
}

/** 发起扫码登录（返回二维码内容，UI 负责渲染）。 */
export function beginLogin(method: "open" | "cookie"): Promise<{ qr: string }> {
  return call("begin_login", { method });
}

/** 列出远端目录。 */
export function listDir(dirId: string): Promise<
  Array<{ id: string; name: string; isDir: boolean; size: number; mtime: number }>
> {
  return call("list_dir", { dirId });
}
