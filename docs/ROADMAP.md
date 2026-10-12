# CloudMount 路线图

> 2026-10 基线。原则：**每阶段都出可用物**；M0 尽早申请 115 开发者资质
> （审核约 7+7 工作日，是唯一的外部阻塞项）。

## M0 · 打通官方 API（1–2 周）

- [ ] 申请 115 开放平台个人开发者 + 创建 PKCE 应用（外部审核，立即启动）
- [x] `cloudmount-cli login`：PKCE(S256) 设备码扫码 → 终端二维码渲染 → 轮询换取 token
- [x] 凭据持久化：Windows 凭据管理器 / macOS Keychain / Linux 0600 文件
- [x] token 自动刷新（单飞 + 5 分钟提前量 + 40140123–26 失效重试一次）
- [x] `ls`（分页 + usertime 排序）、`quota`（rt_space_info 解析）
- [x] `upload`：SHA1/前128K 预计算 → 秒传命中 → 二次认证（sign_check 闭区间）→ OSS 直传
- [x] `download`：downurl 直链 + Range 流式下载（带进度）
- [x] 全局限速（1 r/s 令牌桶）+ 指数退避 + 频控码识别（406 / 40140117）
- [ ] **待 AppID 实测**：真实账号跑通全链路；确认 copy/新建目录的响应字段
- **验收**：CLI 全流程可用；请求速率 ≤1 r/s 无频控

## M1 · Rust 同步引擎（3–5 周）

- [x] SQLite（bundled）元数据库：上次同步快照（三向对比基准）、状态机、pin 策略
      （`水合范围位图` 属 VFS/占位符层，随 M2 落地）
- [x] 本地监控抽象 `LocalWatcher` + 可移植轮询基线（`PollingWatcher`）
      ；USN/FSEvents/inotify 原生实现待接入（M1.5）
- [x] 远端变更发现：`change_probe` 廉价探测（1 请求）+ 全量扫描兜底 + 水位短路
      （短路时**用快照合成远端视图，零远端请求**）
- [x] 三向对比 `reconcile::plan`：纯函数、确定性输出、可穷举测试；
      冲突默认"保留两者"，删除传播默认关闭（保守，防误删）
- [x] 队列调度：并发/重试退避/暂停；只重试可重试错误（网络/频控/5xx）
- [x] 块缓存：LRU + 字节限额 + 命中统计 + 索引持久化（下载去重生效）
- [x] 执行器：原子写入（临时文件 + rename）、保持远端 mtime、删除进**回收站**
- [x] `cloudmountd` 无头服务：同步循环 + `--once`（可作 cron/边缘常驻）
- [x] **114 个测试**（54 单元 + 20 端到端 + 40 其它）；端到端用内存 Mock 远端覆盖
      双向同步/冲突/删除策略/幂等/短路/暂停恢复语义
- [ ] 待真实凭据联调：两台设备经 115 完成一次干净的双向同步（依赖 M0 的 AppID）
- [ ] 验收指标：空闲 RSS ≤ 20MB（需真机测量，`cloudmountd --once` 下观测）

**已知边界（诚实记录）**：变更检测基于 (size, mtime) 秒级粒度，**同秒内等大小修改会漏检**
（已有专门测试固定该行为）；缓解方案是 M1.5 的可选内容校验通道或平台原生 watcher。

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

**实现方案已定稿**：[RUST-MINIFILTER-DESIGN.md](../drivers/win/RUST-MINIFILTER-DESIGN.md)
—— 用 Rust 从零实现（cdylib 直出 `.sys`，自建 FltMgr 绑定层 + 薄 C 壳回退路径），
含 14 项稳定机制与 S1–S6 门禁、KM-0…KM-7 里程碑（18–30 人月，验证占半）。

关键结论：不再走"C 内核 + Rust 用户态"；官方两条红线（HSM 组内不得对 recall 文件
做 targeted 缓存读写、必须处理 section 同步钩子）已写入设计；零售分发需过
HLK/WHCP（2026 签名政策收紧）。**不阻塞任何产品里程碑。**

## 里程碑依赖

```
M0 ──→ M1 ──→ M2（Windows 首发）──→ M3 ──→ M5
              └──→ M4（边缘与 M2 可并行）
```
