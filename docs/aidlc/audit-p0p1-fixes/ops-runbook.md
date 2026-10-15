# Ops Runbook: audit-p0p1-fixes

> 产出者：编排者　日期：2026-10-15　前置：gate2 approved
>
> 人类裁决（AskUserQuestion 2026-10-15，逐字留痕 decisions.md）：**监控与 SLA = 本地轻量**；**告警阈值 = 硬信号即时，warn 巡检**；**部署窗口 = 手动发布+发布前全腿**。

## 系统画像

本地挂载工具，无服务端集群、无线上 SLA 对象：`cloudmount-daemon`（Windows SCM 服务 + WebDAV serve + VFS 水合）、`cloudmount-cli`、`apps/desktop`；云端为 115 网盘（cookie 通道 / open 通道，`--channel auto` 经 ProviderRouter probe 探测切换）。可观测面 = CI 门 + SCM 服务恢复 + `tracing` 日志文件。

## 监控体系（裁决：本地轻量，零新增基建）

1. **CI 三平台双腿绿 = 发布硬门**：`.github/workflows/ci.yml`——`rust` 主腿（fmt --check / check / clippy -D warnings / test）+ `rust-mock-features` 并列腿（webdav_serve 安全集成真实执行）+ `web-ui` job；矩阵 ubuntu / windows / macos。
2. **Windows SCM 服务恢复**：daemon 服务失败由 SCM recovery 自动重启；重启无效才需人工。
3. **日志 warn 人工巡检**：每月一次或排障时按下方信号清单过一遍日志文件。

## 可观测信号清单（本 intent 修复面，巡检对照用）

| 信号 | 位置 | 巡检要点 |
|---|---|---|
| probe 通道切换 | `last_switch` 记录（router.rs） | `--channel auto` 频繁切换 → 某通道持续 Auth 失效，查 cookie 有效期/refresh 链 |
| watcher detach+warn | rdc.rs 超时分支 | 目录监控线程 5s 未干净退出走人工验证兜底——应罕见，现身即查 |
| 目录事件缓冲溢出 | rdc.rs:191（全量重扫 warn） | 频繁出现 = 事件风暴，评估监控目录规模 |
| spool 残留 warn | webdav.rs 删除失败接受残留 | 残留文件会累积，巡检时手动清理并记录次数 |
| Provider 调用失败 / 本轮失败操作计数 | webdav.rs:178 / daemon main.rs:670 | errors 计数连续多轮非零 → 查云端通道健康 |
| oss_put 显式 Err | `Error::Http{status, 摘要}` | 4xx 快速失败属预期；5xx 连续出现查 OSS 状态 |
| 配置 fail-fast | 启动 Err「解析失败」+路径 | 硬信号，见下节 |
| 凭据明文 | 任何日志级别 | **出现即安全回归**：F-04 七处脱敏 + trace 全开也不得见金丝雀明文（cookie_logging 断言守护该不变量） |

## 告警阈值（裁决：硬信号即时，warn 巡检）

**硬信号——出现即人工介入，不等巡检：**
1. CI 任一腿红（含 fmt/clippy 步骤）；
2. SCM 服务非预期退出（SCM recovery 重启之外的失败）；
3. 启动期配置 fail-fast Err（坏配置拒绝启动是 F-13 预期行为，但需人工修配置）；
4. 日志出现凭据明文（任何级别）——最高优先，按安全回归处置。

**warn 类**：不设数值阈值；按月巡检评估趋势，突增或新形态再深入，不做无依据的告警扩面。

## 部署流水线（裁决：手动发布 + 发布前全腿）

当前 CI 仅构建+测试，无发布流水线。发布流程（人工触发，无固定窗口）：

1. **发布前检查清单**：
   - [ ] CI 三平台双腿全绿（主腿 + mock 腿 + web-ui）；
   - [ ] 本机冒烟：daemon 启动（SCM/前台）→ 挂载 → 读写 → 卸载；desktop 登录走一次「点击即同意」口径；
   - [ ] 顺带 `cargo test -p cloudmount-desktop`（T-28/T-30 共 4 测试 CI 不可见，人工清单⑦兜底）；
   - [ ] 产物三件齐：daemon / cli / desktop。
2. **触发**：人工打 tag；未来可加 release workflow 自动化三平台 `cargo build --release` + 打包（retro 候选项）。
3. **回退**：回退到上一 tag 产物；SCM 服务重装旧版本。

## 已知观察项与 P2 backlog（巡检/下个 intent 候选）

- **间歇 flaky**（driver115 --lib，本轮 31+ verifier 5 跑零复现）：CI 现身按失败测试名定位；静态头号嫌疑 `file_path_hashing_matches_bytes`（crates/cloudmount-driver115/src/hash.rs:157 固定共享 temp 路径，per-run 唯一目录可根治）。
- **P2 backlog**：exec.rs 错误路径 dir_creating 条目残留（增长上界=失败过的不同前缀数）；webdav.rs:513 冗余条件；cookie_mock/mock_api ~60 行平行实现；cookie.rs:584 initupload 解密明文日志收口为 status/pickcode。
- **环境备注**：`cargo test -p cloudmount-sync`（无 features）编译失败为既有状态（tests/common/mod.rs 无门控），sync 腿需 `--features testutil`。

## 回流机制（Operations 协议 §2）

- 生产事件与后续学习**不改本 intent 任何原始工件**；经 `/aidlc:inception` 写入**新 intent 的 intent.md**（审计链原样保留）。
- 本 intent 的过程学习全部沉淀于 `retro.md`。
