# Plan: audit-p0p1-fixes

> 产出者：编排者（Sprint Planning）　日期：2026-10-14　前置：gate1 approved（@ 671de97）
>
> 依据：prd v3（AC1–AC16）、architecture v3（F-01…F-14 + 16 决策 + desktop 落点）、test-strategy v3（T-01…T-30，30 点）。
> TDD 铁律：每 unit 先提交失败测试（红）→ `/aidlc:lock-tests` 置锁 → 实现转绿。锁范围为全 unit 失败测试文件并集（只增不减）。
> 统一验证命令：`cargo test --workspace` ＋ `cargo test --workspace --features cloudmount-daemon/mock`（两腿全绿为 gate2 前置）。

## Units

| # | Unit | 目标 | 涉及文件 | 测试 | 验收 |
|---|---|---|---|---|---|
| U1 | F-14 测试基建（关键路径） | cookie 通道注入面（with_base_urls 完整基址含 scheme / ecdh_remote_pub 替换 / limiter 覆盖）+ ec115 test_peer ECDH 对端 + driver115/tests/ cookie mock 服务器 | `crates/cloudmount-driver115/src/cookie.rs`、`ec115.rs`、`tests/`（新增 cookie mock） | T-23、T-24 | AC14 全矩阵绿（uploadinfo/gettoken/oss_put/initupload 错误路径 + 成功回环 + 秒传命中/未命中）；默认渲染逐字节等价（既有测试不回退） |
| U2 | F-04 凭据脱敏（五处） | cookie.rs:401/492/560/796 字段级脱敏（redact_fields：字段名+长度）+ cli main.rs:233 cookie 回显脱敏 | `crates/cloudmount-driver115/src/cookie.rs`、`crates/cloudmount-cli/src/main.rs` | T-07 | AC4：trace 全开捕获，五处无金丝雀明文（含非 2xx 失败路径） |
| U3 | F-11 oss_put 显式失败 | 非 2xx → `Err(Error::Http{status, 摘要})`（体 take(2048)） | `crates/cloudmount-driver115/src/cookie.rs`、`error.rs`（如需） | T-19、T-20 | AC11：403/500 必 Err 且含状态码；2xx + verify 判成功；5xx 可重试、4xx 快速失败 |
| U4 | F-10 双入口单飞刷新 | `refresh_locked(skip_if)` 持锁助手 + `refresh_stale(&token)`；client.rs:244 改调；passport_base doc(hidden) 注入 | `crates/cloudmount-auth/src/open.rs`、`crates/cloudmount-driver115/src/client.rs` | T-17、T-18 | AC10：10 线程会合刷新恰 1 次（access_token 与 refresh 双入口）；失败全体一致 Err、锁释放 |
| U5 | F-05 + desktop 回接 | core `[cookie]` 配置段 + `set_cookie_consent` 纯函数；daemon 两处硬编码废除（check_enabled 把关，auto 静默跳过）；CLI 登录写回；desktop 勾选初值/点击构造/写回接线 | `crates/cloudmount-core/src/config.rs`、`crates/cloudmount-auth/src/cookie.rs`（如需）、`crates/cloudmount-daemon/src/main.rs`、`crates/cloudmount-cli/src/main.rs`、`apps/desktop/src/main.rs` | T-08、T-27、T-28、T-30 | AC5 三态 + AC16 三分支（点击即同意；组装路径硬编码零命中 + 登录点注释豁免） |
| U6 | F-02 probe 转发 | `impl Provider for ProviderRouter` 补 `change_probe` 转发（经 call() 获得切换语义） | `crates/cloudmount-driver115/src/router.rs` | T-03、T-04、T-26 | AC2 三分支：Some/None 透传、Auth 失效切换（last_switch 断言）、非认证 Err 透传 |
| U7 | F-06 RdcWatcher 真释放 | 所有权重构（Option 字段）+ RawHandle::close 幂等 + exited 标志 + join 5s 超时 detach+warn + ACTIVE_WATCHERS（spawn 后 +1，saturating_sub -1） | `crates/cloudmount-sync/src/rdc.rs` | T-09、T-10 | AC6：50-100 轮循环计数回落基线（windows 腿：drop 后可删被监视目录） |
| U8 | F-07 锁内计划/锁外执行 | ensure_remote_dir 两阶段 + per-前缀锁 + 双检 | `crates/cloudmount-sync/src/exec.rs` | T-11、T-12 | AC7：时间窗交叠主判据 + 并发同目录 create_dir 恰一次 |
| U9 | F-01 水合分块 | HydrateFn → ChunkSink 契约 + `pump_chunks` 可测函数（1 MiB 复用缓冲）+ FETCH_DATA 分块 + 取消语义 | `crates/cloudmount-vfs/src/backends/cfapi.rs`、`cfapi_mount.rs` | T-01、T-02 | AC1：reader 级读取序列除末块外恒 ≤1 MiB；内容逐字节一致；短读/取消干净失败 |
| U10 | F-03 路径遍历 + CI job | serve_static 成分级校验 + canonicalize 双检；webdav_serve harness 扩 `start_server_with_ui` + Provider 注入口；ci.yml 增 `--features cloudmount-daemon/mock` job | `crates/cloudmount-daemon/src/webdav.rs`、`tests/webdav_serve.rs`、`.github/workflows/ci.yml` | T-05、T-06、T-25 | AC3：攻击向量全 4xx 不泄漏金丝雀、正路径 200；CI mock 腿生效 |
| U11 | F-08+F-09 WebDAV 写路径 | flush 幂等 + take() + flushed 置位 + Drop 自愈 + 删除失败 warn 接受残留；写打开仅 NotFound 合成新条目 | `crates/cloudmount-daemon/src/webdav.rs`、`tests/webdav_serve.rs`（包装族） | T-13、T-14、T-15、T-16 | AC8/AC9：spool 零残留且恰一次上传；共享冲突接受残留；非 NotFound 上抛且 upload==0 |
| U12 | F-12+F-13 SCM 接线 + 配置 fail-fast | Run 分支按 placeholder_mount 分派（stop_check 注入）；service.rs 删死 FFI 入口；build_launch_args 跨平台纯函数 + --interval 烘焙；配置加载 fail-fast | `crates/cloudmount-daemon/src/main.rs`、`service.rs` | T-21、T-22、T-29 | AC12/AC13：两路径配置一致；死符号零命中；--interval 快照；坏配置 Err 含「解析失败」+路径 |

## 执行顺序与并行泳道

- **泳道 1（driver115 关键路径）**：U1 → U2 → U3；U6（router.rs 独立文件，可插队并行）
- **泳道 2（auth）**：U4
- **泳道 3（sync）**：U7、U8（不同文件可并行）
- **泳道 4（vfs）**：U9
- **泳道 5（daemon 串行）**：U10 → U11 → U12（U10/U11 同文件 webdav.rs，U10 先行建 harness）
- **泳道 6（跨 crate）**：U5
- **波次**：①U1+U4+U9（三 crate 并行）→ ②U2+U5+U7+U10 → ③U3+U6+U8+U11 → ④U12 → ⑤全量验证（两腿）→ gate2

## TDD 锁记录

- 待置位：各 unit 失败测试提交后由编排者运行 `/aidlc:lock-tests`，`tddLockPaths` 取并集（只增不减）；契约错误走 `/aidlc:unlock-tests <理由>` 并留痕 decisions.md。

## r3 残留微调回填

- 8 条 MINOR 已于 Sprint Planning 时由编排者按终审决议顺带回填（prd 3 处、test-strategy 5 处，含 T-30 注释断言、T-11 副判据场景归属、T-08 锚统一、T-05 向量几何更正、版本标签 v3、AC1 末块口径、AC16 括注更正、已裁决节溯源标注）——见 decisions.md 对应条目。
