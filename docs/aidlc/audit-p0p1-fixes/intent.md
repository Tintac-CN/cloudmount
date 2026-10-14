# Intent: audit-p0p1-fixes

> 创建：2026-10-14　状态：inception

## 一句话意图
"请按照审计报告开始修复"

## 问题与期望
- **要解决的问题**：v0.1.0 发布前完成的双代理审计（aidlc:reviewer-architect + aidlc:reviewer-dev，2026-10-13/14）发现 1 项 P0、13 项 P1 缺陷，覆盖：
  - **内存安全**：CfAPI 水合回调把整个请求范围 read_to_end 进内存（4GB 视频 → 4GB Vec）
  - **路径遍历**：WebDAV serve_static 仅检查 `..`，Windows 上 `/C:/foo` 逃逸 root.join
  - **凭据泄漏**：cookie.rs 三处 debug! 明文打印 userkey/STS 原始响应
  - **资源泄漏**：RdcWatcher Drop 为 `drop(self.handle.clone())` 净变化为零；WebDAV spool flush 未 take() 且 Windows 共享冲突下 remove_file 失败
  - **并发正确性**：OpenApiAuth 刷新在持锁前释放、并发双刷触发 40140117；ensure_remote_dir 持锁做网络 IO
  - **错误混淆**：oss_put 吞 HTTP 状态；WebDAV 写打开把所有 Err 当"新文件"
  - **功能失效**：SCM 服务路径从不检查 placeholder_mount；ProviderRouter 未转发 change_probe（auto 通道丢快路径，100 目录 @1r/s → 每周期 100+ 秒）
  - **配置静默降级**：daemon 配置加载失败不报错、回退默认值
  - **测试缺口**：Cookie 通道零 HTTP 级测试
- **期望的结果**：全部 P0+P1 以 TDD 方式修复（先失败测试后实现），全量回归 + CI 三平台绿；P2 记录在案另行安排，不在本 intent 范围。

## 受影响系统/用户
- `cloudmount-vfs`（cfapi / cfapi_mount 水合路径）——桌面挂载用户
- `cloudmount-daemon`（webdav 导出 / service / main）——WebDAV 与 Windows 服务用户
- `cloudmount-driver115`（cookie 通道 / router）——Cookie 通道与 auto 路由用户
- `cloudmount-sync`（rdc / exec）——实时同步引擎
- `cloudmount-auth`（open 刷新）——官方 OpenAPI 通道
- 安全敏感面：WebDAV 边缘模式暴露面、日志产物中的凭据

## 约束
- Rust 工作区（crates/* + apps/desktop），AGPL-3.0-or-later，遵循既有 crate 边界与模式（brownfield）
- **TDD 铁律**：每个缺陷先提交失败测试，`/aidlc:lock-tests` 置锁后实现
- 不改变对外行为语义（缺陷本身即为错误行为的除外）
- 内存效率目标不回退（空闲 RSS 5-15MB 目标）
- CI 三平台绿为完成判据之一
- 审计输入：`docs/aidlc/audit-p0p1-fixes/audit-findings.md`（编排者从双代理审计回报逐字归档）

## 开放问题（需人类裁决）
- [ ] **范围**：默认本 intent 只覆盖 P0+P1（1 P0 + 13 P1，其中 1 项为测试缺口随修），约 26-37 项 P2 延后到后续 intent —— Gate#1 时可调整
- [ ] **交付方式**：默认单 intent 连续施工（unitStatus 断点续跑），不按 crate 拆多 intent
