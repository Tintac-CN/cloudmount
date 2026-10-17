/**
 * CloudMount API 面（前后端契约）。
 *
 * 唯一实现方为边缘 cloudmountd（POST /api/{cmd}，见 daemon webdav.rs /api/*）；
 * 桌面端已切换为纯 Rust egui 壳（apps/desktop），不经本契约。
 * 命名与参数保持一致。8 命令已全部实现。
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
  /** 累计上传（引擎线程累加）。 */
  totalUploaded?: number;
  /** 累计下载。 */
  totalDownloads?: number;
}

/** 内容块缓存统计。 */
export interface CacheInfo {
  bytes: number;
  limitBytes: number;
  hits: number;
  misses: number;
  inserts: number;
  evictions: number;
}

/** 云端配额。 */
export interface QuotaInfo {
  total: number;
  used: number;
  remaining: number;
}

/** 查询同步与账号状态（托盘/主界面共用）。 */
export function getStatus(): Promise<StatusInfo> {
  return call<StatusInfo>("get_status");
}

/** 缓存统计（占用/命中率/写入/淘汰）。 */
export function getCacheStats(): Promise<CacheInfo> {
  return call<CacheInfo>("get_cache_stats");
}

/** 清空内容缓存（一键清理；元数据不受影响）。 */
export function clearCache(): Promise<void> {
  return call("clear_cache");
}

/** 云端配额。 */
export function getQuota(): Promise<QuotaInfo> {
  return call<QuotaInfo>("get_quota");
}

/** 导出最近日志（行数组）。 */
export function exportLogs(): Promise<string[]> {
  return call<string[]>("export_logs");
}

/** 设置同步根目录（设置页）。 */
export function setSyncRoot(path: string): Promise<void> {
  return call("set_sync_root", { path });
}

/** 暂停/恢复同步（返回新状态：true = 已暂停）。 */
export function togglePause(): Promise<boolean> {
  return call<boolean>("toggle_pause");
}

/** 打开同步目录（资源管理器）。 */
export function openSyncFolder(): Promise<void> {
  return call("open_sync_folder");
}
