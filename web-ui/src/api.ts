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
  syncing: boolean;
  paused: boolean;
  pendingUploads: number;
  pendingDownloads: number;
  conflicts: number;
  errors: number;
  lastMessage: string;
}

/** 查询同步与账号状态（托盘/主界面共用）。 */
export function getStatus(): Promise<StatusInfo> {
  return call<StatusInfo>("get_status");
}

/** 设置同步根目录（设置页）。 */
export function setSyncRoot(path: string): Promise<void> {
  return call("set_sync_root", { path });
}

/** 暂停/恢复同步（返回新状态：true = 已暂停）。 */
export function togglePause(): Promise<boolean> {
  return call("toggle_pause");
}

/** 打开同步目录（资源管理器）。 */
export function openSyncFolder(): Promise<void> {
  return call("open_sync_folder");
}
