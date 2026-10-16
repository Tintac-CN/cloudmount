# CloudMount 路线图与实现审核报告（audit-findings）

> **元信息**
> - **intent**：`audit-roadmap-next-steps`（审核+路线图；gate1 裁决②③：交付=审核+路线图、证据标准=全量验证）
> - **scope**：`docs/ROADMAP.md` 全部 `[x]` 完成声明（实读清点 **43 条**：M0×7／M1×9／M1.5×7／M2 段（含段内 M2.5 条目）×12／M3×3／M4×2／M5×3）+ 长期段叙述性声明 + 已见苗头偏离 + 偏离扩展扫描 + 开放项/backlog 归集 + 质量与交付面盘点 + 下一阶段路线图
> - **方法（全量验证口径声明）**：逐 `[x]` 条目 + 句内子声称落证据；每条给出真实性三态判定〔**已证实／漂移／不可证**〕+ 严重度分级〔P0=关键声明失实或阻塞推进／P1=文档-实现漂移需回写／P2=记录性偏差〕+ 可复现证据（`文件路径:行`、命令、输出片段）；真机/外部项标「声称·未独立复现」。**AC1 附注（长期段形态出入）**：「长期」段无 checkbox（叙述性「已定稿」段），核对象为文字声明，由 U2/TP-12 承接，见 §1.4。
> - **基线**：`cargo test --workspace` = **277 passed / 0 failed**（plan.md 2026-10-16T21:26:42+08:00 录制；本 U1 复跑逐数一致，见 §6/TP-03）；五断言脚本 112/72/23/13/9（U3/TP-13 复跑复核）
> - **执行分工**：U1（本报告 §1 的 M0/M1/M1.5，TP-01..06+28/29）→ U2（§1 的 M2–M5/长期续写）→ U3（§2/§4）→ U4（§0/§3/§5 收口）
> - **日期**：2026-10-16　**执行者**：AIDLC construction（dev 角色，U1 起笔）
> - **零写入声明**：本报告为审核期唯一仓库写入物（prd 非目标「审核期零写入」）；`cargo test` 瞬态测试夹具按 test-strategy M10 执行面声明显式接受（git 面零写入不受影响）

---

## §0 摘要与计数矩阵（待 U4 补齐）

（占位：U4 汇总三态×严重度计数矩阵与头号方向建议一句话；最终矩阵以全量 43 条 `[x]` + 长期段声明收口，并与 §1–§5 明细行数机械一致〔AC15⑤〕。U1 已核 **23 条**（M0×7/M1×9/M1.5×7），当前分布：**已证实 23／漂移 0／不可证 0**，随附独立小发现 4 条（全部 P2，见 §1 各表「小发现」行）；最终矩阵待 U2/U3 产出后由 U4 填写。）

---

## §1 里程碑断言核验

### §1.0 方法与判定口径基线（AC1）

- **核验对象**：`docs/ROADMAP.md` 逐 `[x]` 条目 + 条目内句内子声称；测试计数声明（M1 L33-34 / M1.5 L58-59）单列。
- **判定口径双轨**（prd 头注）：真实性三态〔已证实／漂移／不可证〕×严重度〔P0/P1/P2〕，两者并用互不替代。
- **证据标准**：gate1 人类裁决③=**全量验证**——逐条目+逐子声称；每条配可重跑命令或可对照的 `文件路径:行`。
- **测试计数声明口径**（test-strategy 缺口节）：复跑对照的是「量级与构成」，历史计数声明核验的是「当时存在」——历史 114/131 无法从当前 HEAD 逐字复现（HEAD 已含后续 intent 新增面），判定按双口径各自量级一致 + 口径差如实注记。
- **命名微差口径**（gate1 M1 修订）：ROADMAP 协议措辞（如「PKCE(S256)」「usertime」）与实现命名（`code_challenge_method="sha256"`、`o=user_utime`）的出入列为**独立小发现（P2）**，不作一票证伪。
- **执行面**：本机=M2 以来开发机（plan.md 注记「无环境漂移」）；cfapi 集成腿真实执行并产生瞬态夹具副作用（显式接受）；inotify 腿 Linux-only，本机不可执行——以静态符号+CI Linux 矩阵（ci.yml:20）为结构性证据。

### §1.1 M0 · 打通官方 API（TP-01+TP-02，7/7 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据（详见 §6/TP-01、TP-02） |
|---|---|---|---|---|
| L9 | `cloudmount-cli login`：PKCE 设备码扫码 → 终端二维码渲染 → 轮询换取 token | **已证实** | — | `Commands::Login` 注册 @ `crates/cloudmount-cli/src/main.rs:47,114`；`code_challenge`/`code_challenge_method` @ `crates/cloudmount-auth/src/open.rs:308-309`；PKCE 计算 @ `crates/cloudmount-auth/src/pkce.rs:3-4,61`；qrcode 渲染 @ `main.rs:584-585`（依赖 @ `crates/cloudmount-cli/Cargo.toml:24`）；`begin_login` @ `open.rs:300`、`poll_login` 长轮询 @ `open.rs:332`（流程注记 `open.rs:7-10`）。子声称：设备码扫码✓／二维码渲染✓／轮询换 token✓。**小发现①（P2）**：ROADMAP「PKCE(S256)」vs 实现 `code_challenge_method="sha256"`（`pkce.rs:4`）——协议措辞 vs 代码命名，gate1 M1 修订口径，非证伪 |
| L10 | 凭据持久化：Windows 凭据管理器 / macOS Keychain / Linux 0600 文件 | **已证实** | — | `default_store()`：windows/macos→`SystemStore`（keyring）@ `crates/cloudmount-auth/src/store.rs:192-201`（服务名 `io.github.tintac-cn.cloudmount` @ :145）；其余平台（Linux/边缘）→`FileStore` 0600 @ :75-122 + `restrict_permissions(0o600)` @ :125-137（仅 Unix 强制）。子声称三平台✓。**小发现②（P2）**：`store.rs:5` 模块头注提及「Linux：Secret Service（libsecret）」但 `SystemStore` 仅 `cfg(any(windows, target_os="macos"))`（:140）——Linux 实走 0600 文件（与 ROADMAP L10 一致），头注属未落地的候选表述 |
| L11 | token 自动刷新（单飞 + 5 分钟提前量 + 40140123–26 失效重试一次） | **已证实** | — | 提前量 `REFRESH_MARGIN_SECS=300` @ `open.rs:39`（`needs_refresh` @ :54-56）；单飞（同锁合并并发刷新）@ `open.rs:178,218-223`；失效码域 `40140123..=40140126→TokenExpired` @ `crates/cloudmount-core/src/envelope.rs:71`；**重试一次** @ `crates/cloudmount-driver115/src/client.rs:216-250`（`with_token_retry`：强制刷新后重试一次，:243-250）；40140117→`RateLimited` 不重试 @ `envelope.rs:72` + `client.rs:218`。子声称三项逐项✓ |
| L12 | `ls`（分页 + usertime 排序）、`quota`（rt_space_info 解析） | **已证实** | — | 分页循环（`PAGE_LIMIT`/`offset`/`MAX_PAGES` 防异常）@ `crates/cloudmount-driver115/src/open.rs:206-246`；排序 `o=user_utime&asc=0` @ `open.rs:225-226`；`rt_space_info` 解析 @ `crates/cloudmount-driver115/src/models.rs:211-267`（缺字段即报错 :265-267）；CLI `Ls` @ `main.rs:64,129,440`、`Quota` @ `main.rs:70,130,471-474`。**小发现③（P2）**：ROADMAP「usertime」vs 实现参数 `user_utime`（同一字段族，`open.rs:421` 注释自证「与 list() 同一排序口径」） |
| L13 | `upload`：SHA1/前128K 预计算 → 秒传命中 → 二次认证（sign_check 闭区间）→ OSS 直传 | **已证实** | — | 子声称逐项：前 128KiB 预计算 `PREID_CHUNK=128*1024` @ `crates/cloudmount-driver115/src/hash.rs:14`（preid 大写 SHA1 @ :5,24-25,43-44,75）；秒传命中 status=2 零传输 @ `open.rs:326-341`；sign_check **闭区间** `[start,end]` @ `hash.rs:82-91`（`end_inclusive`）+ status 6..=8 二次认证重试 @ `open.rs:343-360`；OSS 直传 `upload_to_oss`+`x-oss-callback` @ `open.rs:126-166,362`。链路四环节符号齐备（TP-02 证伪判据未触发：sign_check 字面在位） |
| L14 | `download`：downurl 直链 + Range 流式下载（带进度） | **已证实** | — | downurl @ `open.rs:260-271`；Range 流式 `get_range_stream`（`Range` 头 @ `client.rs:179`）@ `client.rs:166-186`；CLI 进度每 16MiB @ `main.rs:540-568`（:562-567「进度: x / y」）。子声称三项✓ |
| L15 | 全局限速（1 r/s 令牌桶）+ 指数退避 + 频控码识别（406 / 40140117） | **已证实** | — | 令牌桶 `RateLimiter` @ `crates/cloudmount-driver115/src/ratelimit.rs:9-64`（默认 `conservative()`=1 r/s 容量 2 @ :40-42）；**全局接线**：`ApiClient.limiter: RateLimiter` 为实例字段（所有请求共享）@ `client.rs:29,55`；指数退避翻倍封顶 @ `ratelimit.rs:66-103`（:92-96）；频控码 `406→RateLimited` @ `envelope.rs:73`、`40140117→RefreshTooFrequent` @ `envelope.rs:72`（分类测试 @ :210-213）。（附注：M5 已拆出 `cdn_limiter` @ `client.rs:32,56`——CDN 下载与 API 限速分离，不与本条冲突） |

#### M0 未勾项标注（AC2 要求）

| ROADMAP 行 | 未勾项 | 阻塞状态 |
|---|---|---|
| L8 | 申请 115 开放平台个人开发者 + 创建 PKCE 应用 | **未勾属实**（`docs/ROADMAP.md:8`）；**外部审核阻塞链源头**（`docs/ROADMAP.md:3-4`「唯一的外部阻塞项」，审核约 7+7 工作日）——审核期无法验证审批进度，状态=外部等待中（test-strategy 缺口节） |
| L16 | 待 AppID 实测：真实账号跑通全链路；确认 copy/新建目录响应字段 | **未勾属实**（`docs/ROADMAP.md:16`）；依赖 L8 出 AppID——真实账号联调**本期未执行**（代码侧就绪度已由上表 7 条证实；`open.rs:274,371` 尚存 `TODO(verify)` 待实测注记，与 L16「确认响应字段」语义吻合——未勾声明诚实） |

### §1.2 M1 · Rust 同步引擎（TP-03+TP-04+TP-28，9/9 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L21-22 | SQLite（bundled）元数据库：上次同步快照（三向对比基准）、状态机、pin 策略 | **已证实**（主声明） | — | bundled @ 根 `Cargo.toml:38`（`rusqlite = { version="0.37", features=["bundled"] }`）+ `crates/cloudmount-sync/src/store.rs:4` 注释；快照记录 `EntryRecord`→`entries` 表 @ `crates/cloudmount-sync/src/state.rs:95-117`；状态机 `SyncState` @ `state.rs:9`；pin 策略 `PinState`（Pinned/Online/Excluded/Inherit 往返）@ `state.rs:61-91`。**子声称漂移（小发现④，P2）**：括号注记「`水合范围位图` 属 VFS 层随 M2 落地」——全仓无位图实现（`grep -rni 'bitmap\|位图' crates/ apps/desktop/src` 仅构建产物与图标注释噪音）；M2 实际以 CfAPI 内核原生检索状态+分块推送承接（`crates/cloudmount-vfs/src/backends/cfapi.rs:22,586`），前瞻注记的**命名工件未以字面落地**，能力语义已被替代方案覆盖 |
| L23-24 | 本地监控抽象 `LocalWatcher` + 可移植轮询基线（`PollingWatcher`）；USN/FSEvents/inotify 原生实现待接入（M1.5） | **已证实** | — | `pub trait LocalWatcher` @ `crates/cloudmount-sync/src/scan.rs:40`；`pub struct PollingWatcher` @ `scan.rs:147`（impl @ :161）；`watcher.rs:28` 导入并作统一差异计算（`diff_snapshots` @ `watcher.rs:175-202`）。括号前瞻由 M1.5 段承接：inotify 已接入（§1.3 L46）、USN 被 RDC 取代（取舍理由 @ `rdc.rs:7-9`）、FSEvents 待 M2+（`watcher.rs:58,93-98`——与 M1.5 段 L48 声明一致，非失实） |
| L25-26 | 远端变更发现：`change_probe` 廉价探测（1 请求）+ 全量扫描兜底 + 水位短路（短路时用快照合成远端视图，零远端请求） | **已证实** | — | 1 请求探测（`user_utime` 倒序 1 条+总数，与 list 同口径）@ `crates/cloudmount-driver115/src/open.rs:418-430`；探测→水位比对→短路 @ `crates/cloudmount-sync/src/remote.rs:4-9,106-115`（短路返回 `None` @ :75）；全量兜底 @ `remote.rs:6`；**快照合成** `synthesize_remote_from_snapshot` @ `crates/cloudmount-sync/src/engine.rs:429-430`（接线 @ engine.rs:214-218「远端未变：用快照合成远端视图」）；测试 `tracker_short_circuits_then_refetches_on_change` @ `remote.rs:294`。子声称全部✓（「零远端请求」= 合成路径无 provider 调用，engine.rs:8 注释+实现吻合） |
| L27-28 | 三向对比 `reconcile::plan`：纯函数、确定性输出、可穷举测试；冲突默认"保留两者"，删除传播默认关闭 | **已证实** | — | `pub fn plan(input: &PlanInput<'_>) -> Vec<Action>` @ `crates/cloudmount-sync/src/reconcile.rs:204`——签名纯引用无 IO/时间/随机（TP-04 证伪判据未触发）；模块注释自证纯函数动机 @ reconcile.rs:2-4；`ConflictPolicy::KeepBoth` 为 `#[default]` @ reconcile.rs:31-32（Default @ :59）；`propagate_remote_deletes`/`propagate_local_deletes` 默认 `false` @ reconcile.rs:52-55,64-66；确定性输出测试 `output_is_sorted_deterministically` @ reconcile.rs:802；穷举测试面 16 个（判定矩阵逐情形：:565-:831，含 `both_modified_conflicts` @ :642、`remote_delete_not_propagated_by_default_resurrects` @ :675、`local_delete_not_propagated_by_default_restores` @ :711） |
| L29 | 队列调度：并发/重试退避/暂停；只重试可重试错误（网络/频控/5xx） | **已证实** | — | `crates/cloudmount-sync/src/queue.rs:1-8`（模块文档三要素）；`SchedulerConfig`（`paused: Arc<AtomicBool>` @ :52-53）；暂停=本轮不取新任务 @ queue.rs:103-104；指数退避 `RetryPolicy` @ :17-44；只重试可重试错误 `e.is_retryable()` @ :123-126（`is_retryable`=`Transport|RateLimited` @ `crates/cloudmount-core/src/error.rs:71`） |
| L30 | 块缓存：LRU + 字节限额 + 命中统计 + 索引持久化（下载去重生效） | **已证实** | — | `crates/cloudmount-sync/src/cache.rs:1-11`（LRU+限额+统计，n 数百量级设计取舍）；`CacheStats`（hits/misses/bytes/limit_bytes/命中率）@ cache.rs:22-44；`BlockCache` @ :69-72；索引持久化 `INDEX_FILE="index.json"` @ :76（载入 @ :84、落盘 @ :217、测试 `index_persists_across_reopen` @ :293）；**下载去重接线**：执行器持 `BlockCache` @ `crates/cloudmount-sync/src/exec.rs:13,46,74`，`cache_key="sha1:{sha1}"` @ exec.rs:193-194；引擎流程「缓存索引落盘」@ `engine.rs:12` |
| L31 | 执行器：原子写入（临时文件 + rename）、保持远端 mtime、删除进**回收站** | **已证实** | — | `crates/cloudmount-sync/src/exec.rs:4-6`（模块文档三声称）；下载先写 `tmp_path` 临时文件 @ exec.rs:201-212、设 mtime=远端 mtime @ :239-241（避免下轮误判）、原子改名；删除进 `<根>/.cloudmount/trash/<时间戳>/` @ exec.rs:4-5（`trash_local` @ :124,414-417） |
| L32 | `cloudmountd` 无头服务：同步循环 + `--once`（可作 cron/边缘常驻） | **已证实** | — | `once` CLI 字段 @ `crates/cloudmount-daemon/src/main.rs:82`；同步循环 `engine.run_once()` @ main.rs:579（循环体）/ :647-675（`--once` 跑一轮即退出、无 flag 常驻 @ :788-790 注释）；SCM 服务面（`service.rs`）与本条无冲突 |
| L33-34 | **114 个测试**（54 单元 + 20 端到端 + 40 其它）；端到端用内存 Mock 远端覆盖双向同步/冲突/删除策略/幂等/短路/暂停恢复语义 | **已证实**（历史口径，见下方对照小节） | — | 复跑 **277 passed / 0 failed**（26 目标，逐数一致于基线）；构成现值 191 单元 + 86 集成；端到端 Mock 远端语义覆盖面在位（`crates/cloudmount-sync/tests/engine_e2e.rs` 28 测试，含冲突/删除策略/幂等/短路/暂停恢复/重命名各节）。历史 114 无法从 HEAD 逐字复现（HEAD 已含 M1.5–M5 增长面）——按 test-strategy 缺口节口径判「量级与构成一致、零失败」，口径差如实注记 |

#### TP-03 复跑对照小节（M1 计数声明专节）

- **命令**：`cargo test --workspace`（裸命令，testutil 经 dev-dependencies 自动启用；与 U3/TP-14 共享同一次复跑）。
- **复跑结果（2026-10-16，本机）**：**277 passed / 0 failed / 0 ignored**，26 目标（20 实测 + 6 Doc-tests 空目标）。逐目标分解：

| crate / 目标 | passed | crate / 目标 | passed |
|---|---|---|---|
| cloudmount_auth（lib 单元） | 13 | cloudmount_sync（lib 单元，含 rdc 5） | 73 |
| cloudmount-cli（lib 单元） | 3 | engine_e2e（sync 集成） | 28 |
| cloudmount_core（lib 单元） | 19 | exec_remote_dir（sync 集成） | 4 |
| cloudmount_daemon lib（单元） | 4 | rdc_release（sync 集成） | 2 |
| cloudmountd bin（单元） | 17 | cloudmount_vfs（lib 单元） | 8 |
| cloudmount_driver115（lib 单元） | 54 | cfapi_backend / cfapi_hydration / control_dehydrate | 13 / 8 / 3 |
| cookie_channel / cookie_logging / mock_api / single_flight_refresh | 9 / 5 / 6 / 8 | Doc-tests ×6 | 0 |

- **交叉验证**：静态 `#[test]` 清点 unit=195 + integration=86 = 281；inotify_watcher.rs 4 测试 `cfg(linux)` 本机不执行 → 281−4=**277**，与复跑逐数吻合（分解自洽）。
- **双口径判读**：历史口径 **114（54 单元+20 端到端+40 其它）**＝M1 时点存在性声明——当前树无法逐字复现（后续 intent 新增 M1.5/M2/M3/M4/M5 面，如 cfapi_* 24 测试、cookie_* 14 测试、rdc_* 7 测试均为 M1 之后落地）；现值 277 ≥ 114 且零失败——**量级一致偏上、构成增长可解释，声明站得住**（TP-03 证伪判据「failed>0 或计数显著低于声称且无口径解释」未触发）。现值 **277**＝desktop runbook 基线口径，逐数一致。
- 附注（M1 已知边界非 `[x]` 声明，随 AC3 一并核）：「同秒内等大小修改会漏检」专门固定测试在位 @ `crates/cloudmount-sync/src/scan.rs:246-259`（`same_second_same_size_edit_is_not_detected`）；该盲区是否被 M1.5 正式关闭 → 见 §1.3 L49-51（**已关闭，证据在位**）。

### §1.3 M1.5 · 原生 watcher + 内容校验 + 重命名（TP-05+TP-06+TP-29，7/7 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L43-45 | Windows 原生 watcher（`ReadDirectoryChangesW`）：无需管理员权限（USN 需提权…选 RDC 原因）、一个句柄监听整棵子树、后台线程 + 脏路径归约 + 溢出/周期性全量对账自愈；本机真实 API 测试 5/5 通过 | **已证实**（5/5 已本机独立复跑） | — | `crates/cloudmount-sync/src/rdc.rs:1-10`（模块文档：无需提权 vs USN 提权取舍、一个句柄整棵子树、后台线程→脏路径批次）；溢出标志 @ rdc.rs:59、溢出→全量重扫 @ :189-195、周期对账 `FULL_RECONCILE_EVERY_N_POLLS` @ :33,195；脏路径应用 `apply_dirty` @ `watcher.rs:103-154`。**5/5 复跑**：`cargo test -p cloudmount-sync --lib rdc` → `5 passed; 0 failed`（真实 ReadDirectoryChangesW 对真实临时目录执行；本机=开发机）。子声称逐项✓ |
| L46-47 | Linux 原生 watcher（inotify）：递归补挂 watch、`Q_OVERFLOW` 自愈、唤醒文件优雅退出（`.cm-wake.cmtmp`，扫描自动过滤） | **已证实**（代码+CI 结构性证据；Linux 运行时本机不可执行，如实注记） | — | `crates/cloudmount-sync/src/inotify_watcher.rs:8-13`（递归补挂/Q_OVERFLOW 全量重扫/唤醒退出三声称集中自证）；`WAKE_FILE=".cm-wake.cmtmp"` @ :32-33；扫描过滤 `is_internal`（`.cmtmp` 后缀）@ `crates/cloudmount-sync/src/scan.rs:118-120`（测试 @ :212-217）；Linux-only 测试 4 个 @ inotify_watcher.rs（本机 cfg 排除，**CI Linux 腿承担**：`.github/workflows/ci.yml:20` `ubuntu-latest` ∈ rust/rust-mock-features 两矩阵） |
| L48 | macOS：暂用轮询（FSEvents 待 M2+），`build_watcher` 工厂自动回退 | **已证实** | — | `build_watcher` @ `crates/cloudmount-sync/src/watcher.rs:60-69`；macOS 分支（非 win 非 linux）→ 轮询 @ watcher.rs:93-98（「当前平台无原生 watcher，使用轮询扫描」）；回退语义（原生失败→轮询告警）@ :71-91；测试 `polling_backend_builds` @ :370、`auto_backend_falls_back_gracefully_on_missing_root` @ :378 |
| L49-51 | 内容校验通道：每 N 轮按 `content_check` 时间戳轮转哈希校验一批文件，比对本地内容与远端 SHA1——正式关闭"同秒同大小修改"盲区（有专门的盲区回归测试与"关闭校验"行为测试） | **已证实** | — | 轮转参数 `verify_every_cycles`（默认 5 轮）+ 每轮 32 文件 @ `crates/cloudmount-sync/src/engine.rs:65-70,87-90`；按 `content_check` 升序取批 @ engine.rs:352-384（:368 排序、:384 推进水位）；比对远端 SHA1、不一致注入 plan @ engine.rs:235,353,389；**两类测试齐备**（TP-06 证伪判据未触发）：盲区回归 `content_verification_catches_same_second_same_size_edit` @ `crates/cloudmount-sync/tests/engine_e2e.rs:631`（固定 mtime 制造同秒同大小 @ :647-654）+ 关闭校验行为 `content_verification_can_be_disabled` @ engine_e2e.rs:675（:698「关闭校验→盲区修改不被检测=文档化边界」）。**AC3 结论：同秒漏检盲区已被 M1.5 内容校验通道正式关闭**（默认开启 verify_every_cycles=5；关闭属显式可配的文档化行为） |
| L52-56 | 重命名检测（零传输）：本地改名→远端 rename/move（不重传）；远端改名→本地改名（不重下载）。配对条件保守（同步记录+大小相等），执行前哈希二次确认，不匹配自动回退（上传新路径+删旧远端 / 回收站旧文件+下载新文件）——修复"一次重命名变成文件重复" | **已证实** | — | 配对条件（消失方×出现方+**必须有同步记录**+大小相等+只配对文件）@ `crates/cloudmount-sync/src/reconcile.rs:328-347`（动机注释「文件重复」@ :328-330）；本地改名→`move_remote`（`move_entry`+`rename`，零内容传输）@ `crates/cloudmount-sync/src/exec.rs:314-368`（哈希二次确认 @ :320-327，不匹配回退=上传新路径+删旧远端 @ :360-367）；远端改名→`rename_local`（`fs::rename`）@ exec.rs:374-413（哈希确认 @ :378-387，回退=回收站+下载 @ :409-413）；e2e 三测试 @ `engine_e2e.rs:528,575,608`（含 `rename_with_content_change_falls_back_safely`）。子声称五项逐项✓ |
| L57 | 快照 schema v0.2：`content_check` 列（含旧库自动迁移） | **已证实** | — | `migrate_v02_content_check`（幂等 `ALTER TABLE ADD COLUMN`，pragma 探测）@ `crates/cloudmount-sync/src/store.rs:79-99`；字段 @ `crates/cloudmount-sync/src/state.rs:120`（与 architecture.md 数据模型锚 :66 一致） |
| L58-59 | 测试：131 个全过（65 sync 单元 + 27 端到端 + 39 其它），含原生 watcher 与引擎协同的真实 API 测试 | **已证实**（历史口径，复跑对照见下） | — | 复跑现值：sync lib 单元 **73**（≥65，含 rdc 5）、engine_e2e **28**（≥27）、集成总计 86；零失败。协同真实 API 测试在位：`engine_works_with_native_watcher` @ engine_e2e.rs:712 + rdc.rs 5 + rdc_release 2，**本机全过**。历史 131 无法从 HEAD 逐字复现——量级一致偏上、构成增长可解释（同 TP-03 口径），声明站得住 |

### §1.4 长期（待 U2）

（占位：长期段为叙述性「已定稿」声明、无 checkbox——TP-12 全量事实核验由 U2 落此处。）

### §1.5 M2–M5（待 U2）

（占位：M2 段×12（TP-07/08/30）、M3×3（TP-09）、M4×2（TP-10）、M5×3（TP-11）由 U2 续写。）

---

## §2 计划-实现偏离（待 U3）

（占位：U3 按 TP-15..20+24 落「文档/路线图字面｜实际实现｜证据｜建议回写位置」四栏记录表与偏离清单；基线=architecture.md D1–D6 偏离表+苗头对账注记。）

---

## §3 归集表（待 U4）

（占位：U4 按 TP-21/22/23 落开放项归集（AC10）与四 runbook backlog+retro 候选归集（AC11），含来源/去重/分类列。）

---

## §4 质量与交付面盘点（待 U3）

（占位：U3 按 TP-13/14/16/27 落 AC12（测试/CI 现状+缺口清单）与 AC13（已知边界七项逐项三栏结论）。）

---

## §5 下一阶段路线图（待 U4）

（占位：U4 按 TP-25/26 落排序批次表（批次／方向／依赖／风险／价值／建议 intent 边界）+ 批次间依赖；「不发明需求」守护在案。）

---

## §6 证据索引

> 格式：逐 TP → 命令 + 输出摘要 + `文件路径:行`。全部命令可原样重跑（TP-17 复现性核验由 U3 承接）。

### TP-01（M0·登录凭据与查询命令，L9/L10/L11/L12）

- `grep -rn 'code_challenge\|code_challenge_method\|qrcode' crates/cloudmount-auth crates/cloudmount-cli` → 命中 `crates/cloudmount-auth/src/open.rs:308-309`（form 字段）、`crates/cloudmount-auth/src/pkce.rs:3-4,61`、`crates/cloudmount-cli/src/main.rs:584-585`（`qrcode::QrCode` 渲染）、`crates/cloudmount-cli/Cargo.toml:24`。`S256`/`device_code` 字面全仓零命中（预期，gate1 M1 重标定）。
- `grep -n 'Login' crates/cloudmount-cli/src/main.rs` → :47（`Login {` 变体）、:114（`Commands::Login {` 分发）。
- `grep -rn 'usertime\|rt_space_info' crates/` → `crates/cloudmount-driver115/src/models.rs:211-267`（rt_space_info 解析）；排序实际符号 `o=user_utime` @ `crates/cloudmount-driver115/src/open.rs:225-226`（`grep 'user_utime' crates/cloudmount-driver115/src`）。
- 读 `crates/cloudmount-auth/src/store.rs` → keyring 分支 :139-185（`SystemStore`，cfg windows/macos）、0600 分支 :75-122+:125-137（`FileStore`+`restrict_permissions(0o600)`）、默认选择 :192-201。
- 读刷新单飞 → `crates/cloudmount-auth/src/open.rs:39`（`REFRESH_MARGIN_SECS: i64 = 300`）、:178/:218-223（同锁单飞）；`crates/cloudmount-core/src/envelope.rs:69-77`（`classify_code`：40140123..=40140126/40140117/406/700..=702）；`crates/cloudmount-driver115/src/client.rs:216-250`（失效强制刷新后**重试一次**）。
- 长轮询 → `crates/cloudmount-auth/src/open.rs:300`（`begin_login`）、:332（`poll_login`）、:7-10（流程注释「长轮询」）。

### TP-02（M0·上传下载与限速，L13/L14/L15 + 未勾项标注）

- `grep -rn 'sign_check\|sha1\|downurl\|Range\|40140117' crates/cloudmount-driver115 crates/cloudmount-cli` → `crates/cloudmount-driver115/src/hash.rs:5,14,24-25,82-91`（preid 128KiB、sign_check 闭区间 `[start, end_inclusive]`）、`open.rs:260-271`（downurl）、`open.rs:343-360`（status 6..=8 二次认证）、`client.rs:179`（Range 头）、`envelope.rs:72-73`（40140117/406 分类）。证伪判据（sign_check/频控码字面缺失）未触发。
- 令牌桶/退避 → `crates/cloudmount-driver115/src/ratelimit.rs:9-64`（`conservative()`=1 r/s @ :40-42）、:66-103（翻倍封顶）；全局接线 `client.rs:29,55`（`pub limiter: RateLimiter` 实例字段）。
- OSS 直传 → `open.rs:126-166`（`upload_to_oss`+`x-oss-callback`/`x-oss-security-token`）、:362（兜底分支）。
- 进度 → `crates/cloudmount-cli/src/main.rs:562-567`（每 16 MiB「进度: x / y (pct)」）。
- 未勾项 → `grep -n '\- \[ \]' docs/ROADMAP.md` → L8/L16/L35/L36/L133/L154/L181 七项；M0 两个=L8（外部审核阻塞源，ROADMAP.md:3-4）、L16（AppID 实测）。

### TP-03（M1·测试计数声明复跑，L33-34；与 TP-14 共享同一次复跑）

- 命令：`cargo test --workspace`（裸命令）。输出摘要：**`277 passed; 0 failed`** 聚合（26 目标：20 实测+6 空 Doc-tests），与本 intent 基线逐数一致。
- 逐目标分解见 §1.2 对照小节表；静态交叉验证：`for d in crates/*/; do grep -rc '#\[test\]' $d/src $d/tests; done` → unit=195+integration=86=281，减 inotify 4（`cfg(linux)`）=277。
- 双口径：114（L33 历史口径，M1 时点）vs 277（现值基线）——量级一致偏上、增长构成可解释（cfapi_*/cookie_*/rdc_*/webdav_* 等为 M1 后续 intent 落地面），零失败；判「已证实（历史口径+口径差注记）」。

### TP-04（M1·三向对比纯函数，L27-28）

- `grep -n 'pub fn plan\|KeepBoth\|keep_both\|delete_propagation' crates/cloudmount-sync/src/reconcile.rs` → :204（`pub fn plan(input: &PlanInput<'_>) -> Vec<Action>`）、:31-32（`#[default] KeepBoth`）、:59（Default）；删除传播默认关闭 → `propagate_remote_deletes`/`propagate_local_deletes` 默认 false @ :52-55,64-66。
- 穷举测试 → `grep -c '#\[test\]' reconcile.rs`=16（:565-:831 覆盖判定矩阵全部情形+`output_is_sorted_deterministically` @ :802）。
- 纯函数性：签名仅引用输入、无 IO/时间/随机（证伪判据未触发）。

### TP-05（M1.5·watcher 三平台与计数，L43/L46/L48/L58）

- `grep -rn 'ReadDirectoryChangesW' crates/cloudmount-sync/src/rdc.rs` → :1（标题）+:24（import）；取舍理由（USN 需管理员）@ :7-9；溢出/周期对账 @ :33,59,189-195。
- `grep -rn 'inotify\|Q_OVERFLOW' crates/cloudmount-sync/src/inotify_watcher.rs` → :1,8-13,19,33（WAKE_FILE `.cm-wake.cmtmp`）；扫描过滤 `scan.rs:118-120`（`.cmtmp` 后缀）。
- `grep -rn 'build_watcher' crates/cloudmount-sync/src/watcher.rs` → :60-69（工厂）；macOS 轮询回退分支 @ :93-98。
- 计数（读 TP-03 共享复跑）：sync lib 73 / engine_e2e 28 / 集成总 86，零失败——历史 131 量级对照通过。
- **RDC 5/5 独立复跑**：`cargo test -p cloudmount-sync --lib rdc` → `test result: ok. 5 passed; 0 failed`（真实 API 腿，本机执行）。

### TP-06（M1.5·内容校验关闭盲区，L49-51）

- `grep -rn 'content_check' crates/cloudmount-sync` → 字段 @ `state.rs:120`；轮转批次 @ `engine.rs:352-384`（升序排序 :368、推进水位 :384）；迁移 @ `store.rs:79-99`；参数 @ `engine.rs:65-70,87-90`（`verify_every_cycles` 默认 5、每轮 32 文件）。
- 两类测试定位：`grep -rn '盲区\|同秒' crates/cloudmount-sync/tests/` → `tests/engine_e2e.rs:628`（节标题「M1.5：内容校验通道（关闭"同秒同大小"盲区）」）+:631（`content_verification_catches_same_second_same_size_edit`）+:675（`content_verification_can_be_disabled`）；M1 侧行为固定测试 @ `src/scan.rs:246-259`。证伪判据（仅有字段而无专门盲区回归测试）未触发。

### TP-28（M1·剩余七条批量，L21/L23/L25/L29/L30/L31/L32）

- L21：`grep -n rusqlite Cargo.toml` → :38（`features=["bundled"]`）；`state.rs:9,61,95-117`（SyncState/PinState/EntryRecord）。
- L23：`scan.rs:40,147,161`（trait LocalWatcher/PollingWatcher/impl）；`watcher.rs:28,175-202`。
- L25：`open.rs:418-430`（1 请求探测）；`remote.rs:4-9,75,106-115,294`（水位短路+兜底+测试）；`engine.rs:214-218,429-430`（快照合成零请求）。
- L29：`queue.rs:1-8,17-44,47-61,103-104,123-126`；`error.rs:71`（is_retryable）。
- L30：`cache.rs:1-11,22-44,69-76,84,217,293`；下载去重接线 `exec.rs:13,46,74,193-194`。
- L31：`exec.rs:4-6,201-212,239-241,124`（tmp+rename/mtime=远端/trash）。
- L32：`crates/cloudmount-daemon/src/main.rs:82,579,647-675,788-790`（--once/同步循环）。

### TP-29（M1.5·剩余两条，L52/L57）

- 重命名：`reconcile.rs:328-347`（保守配对条件：同步记录+大小相等+只配对文件）；`exec.rs:314-368`（move_remote：哈希二次确认 :320-327、回退「上传+删旧远端」:360-367）、`:374-413`（rename_local：哈希确认 :378-387、回退「回收站+下载」:409-413）；e2e `engine_e2e.rs:528,575,608`。证伪判据（无条件宽泛配对/无哈希确认/无回退）未触发。
- schema v0.2：`store.rs:79-99`（`migrate_v02_content_check` 幂等 ALTER TABLE+pragma 探测）；`state.rs:120`。

---

*（U1 收尾：23/23 条判定齐、证据可复现、骨架 §0–§6 就位。U2 续写 §1.4/§1.5，U3 写 §2/§4，U4 收口 §0/§3/§5。）*
