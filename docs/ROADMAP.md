# CloudMount 路线图

> 2026-10 基线。原则：**每阶段都出可用物**；M0 尽早申请 115 开发者资质
> （审核约 7+7 工作日，是唯一的外部阻塞项）。

## M0 · 打通官方 API（1–2 周）

- [ ] 申请 115 开放平台个人开发者 + 创建 PKCE 应用（外部审核，立即启动）
- [ ] `cloudmount-cli login`：PKCE 设备码扫码 → token 持久化（DPAPI）
- [ ] `ls`（分页 + usertime 字段）、`quota`
- [ ] `upload`：SHA1/前128K 预计算 → 秒传命中 → OSS 分片直传
- [ ] `download`：downurl 直链缓存 + Range 下载
- **验收**：CLI 全流程可用；请求速率 ≤1 r/s 无频控

## M1 · Rust 同步引擎（3–5 周）

- [ ] SQLite（bundled）元数据库：目录树快照、状态机、pin、水合范围
- [ ] 本地监控抽象：USN Journal（Win）/ FSEvents（macOS）/ inotify（Linux）
- [ ] 远端轮询：usertime 头部探测 + 全量兜底 + 退避
- [ ] 队列调度：上传/下载分列、限速、暂停恢复；冲突保留两者
- [ ] 块缓存：LRU、限额、命中率统计
- **验收**：两台设备经 115 完成一次干净的双向同步；空闲 RSS ≤ 20MB

## M2 · Windows 挂载（4–6 周）

- [ ] Tauri 壳：托盘（状态角标/菜单）+ web-ui 集成 + WebView2 引导
- [ ] CfAPI 后端：SyncRoot 注册、占位符、FETCH_DATA 水合、pin/释放空间
- [ ] 资源管理器状态图标与右键动词（CfAPI 原生）
- [ ] MSIX 打包 + 自动更新
- **验收**： explorer 里打开 115 目录即用，行为与 OneDrive 一致

## M3 · Cookie 补充通道（2–3 周）

- [ ] 扫码换 Cookie（独立加密容器）+ 知情同意流程（默认关闭）
- [ ] 只读失效探测（不踢下线）+ 自动回退官方通道
- [ ] Cookie 独有能力：分享/转存/批量操作（二级入口 + 常驻风险提示）
- **验收**：拔掉官方 token 能无感切到 Cookie 并回退；无静默行为

## M4 · 边缘网关（2–3 周）

- [ ] `cloudmountd` 服务模式：WebDAV 出口（rustls）+ web-ui 托管
- [ ] musl 静态交叉编译（aarch64/armv7/x86_64）+ 多架构 Docker
- [ ] 低内存预设 + systemd unit + Tailscale 集成文档
- **验收**：512MB 内存设备稳定运行 48h；局域网设备经 WebDAV 挂载读写

## M5 · 打磨（持续）

- 缓存可视化（占用/命中率/一键清理）、设置页、日志导出
- macOS File Provider 扩展（Swift 薄桥 + core staticlib）
- 性能：万级文件目录投影、并发水合

## 长期 · 自研 Windows minifilter（drivers/win）

按 KM-0..KM-5 推进（见 drivers/win/README.md），与 M2 的 CfAPI 后端并存，
通过 `MountBackend` 切换。**不阻塞任何产品里程碑。**

## 里程碑依赖

```
M0 ──→ M1 ──→ M2（Windows 首发）──→ M3 ──→ M5
              └──→ M4（边缘与 M2 可并行）
```
