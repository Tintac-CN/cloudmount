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

## §0 摘要与计数矩阵（U4 收口）

### §0.1 审核方法声明

**证据标准＝全量验证**（gate1 人类裁决③，覆盖 decisions.md A1 缺省抽样）：`docs/ROADMAP.md` 逐 `[x]` 条目+句内子声称落证据，每条三态判定〔已证实/漂移/不可证〕+严重度〔P0/P1/P2〕+可复现证据（`文件路径:行`/命令/输出片段）；测试计数声明按双口径判读（历史 114/131=当时存在性，现值 277=基线对照）；真机/外部子声称逐处标「声称·未独立复现」（其中 cldflt 金标准/RDC 5/5/万级投影基准三项在本机=开发机真实复跑升级为已复现）。30 个测试点（TP-01..30）全执行，负路径探测器（计数复跑/脚本复跑/死链对账/job 名存在性/边界矛盾检出）零触发证伪。执行分工：U1（§1.0-1.3）→U2（§1.4-1.8）→U3（§2/§4）→U4（§0/§3/§5+AC15 自检），TP-17 自检 3/3 可复现。

### §0.2 计数矩阵（与 §1–§5 明细行数一致，AC15⑤）

| 计数面 | 数值 | 明细落点 |
|---|---|---|
| ROADMAP `[x]` 声明总量 | **43 条**（M0×7/M1×9/M1.5×7/M2 段×12/M3×3/M4×2/M5×3） | §1.1（7 行）/§1.2（9）/§1.3（7）/§1.4（12）/§1.5（3）/§1.6（2）/§1.7（3） |
| ├ 已证实 | **41** | 各段表（真机子声称两项升级为本机复现：cldflt 金标准 8/8、万级投影 693.8ms 复测） |
| ├ 漂移 | **2**（均 P1） | §1.4 L111/L124「Tauri 壳」两行（AC4 漂移结论落字 @ §1.4 专节） |
| └ 不可证 | **0** | — |
| 长期段叙述性声明 | **9 项全部已证实**（该段无 checkbox——AC1 形态出入已在 §1.0 声明） | §1.8 表 9 行 |
| 严重度分布（发现面） | **P0=0／P1=3／P2=9**（计数基=发现条目；P1 归并为 2 个根因簇：Tauri 叙事簇〔L111/L124 两独立条目+L176 子声称，§2.1①〕+web-ui 角色变迁簇〔NEW-1，§2.1④〕） | P1=L111、L124、NEW-1（README 四处 stale Tauri 含 `cargo tauri dev` 死命令）；P2=小发现①..⑤+NEW-2..5 |
| 偏离清单 | 四苗头 4 项分项判定（**为真 2／半边为假 1／非偏离 1**）+ 扩展扫描新增 **5 条**（NEW-1..5）+「无其它新增偏离」显式声明 | §2.1/§2.2/§2.3 |
| backlog 归集 | **37 条**（四 runbook P2 去重 28+观察项 5 组+retro 候选 4；六组重复〔D:n源〕合并；1 条已被后续落地消解标注） | §3.2（37 行） |
| 开放项归集 | **12 项**（ROADMAP 未勾 7+长期 1+本期新增 4） | §3.1（12 行） |
| 下一阶段批次 | **8 批次+长期 1 轨道**（每项带出处；批次依赖图与 ROADMAP.md:196-201 自洽） | §5.1/§5.2 |
| 测试与 CI 现值 | workspace **277/0**；五脚本 **112/72/23/13/9** 全 exit 0；fmt/clippy 双面 clean；CI 缺口 3 条实锚 | §4.1 |
| 已知边界七项 | 2 项被后续里程碑消解（#1 同秒盲区/#7 TLS）、5 项未消解（均刻意取舍或外部依赖） | §4.2 |
| 真机子声称标注 | 43 行已证实中约 **9 行含「声称·未独立复现」子声称**（跨进程验证/内核态 RDC 实测/真机 e2e/脱水实测/待扫码/curl 实测/自签 HTTPS/4 并行 0.31s/内存 23.8MB——见各段表行内标注；其中 cldflt 金标准、RDC 5/5、10k 基准三项已本机复现升级） | §1 各段表 |
| 审核自检 | TP-17 抽样 **3/3 可复现**（§1 行锚零漂移；§3 行锚经 gate2 M4 实测订正后零漂移） | §4.3；§6/TP-21 |

### §0.3 关键结论预览

1. **43 条 `[x]` 声明 41 条属实、零「勾了但没验」实锤**——两处失实均为「Tauri 壳」命名漂移（实现是 egui），功能面在 egui 侧等价存在；路线图整体可信，恢复可信度只需一批文档回写（B1）。
2. **质量交付面健康**：277/0+五脚本全绿+fmt/clippy 双面 clean（含 desktop）；三项真机声称本机独立复现（cldflt 金标准/RDC 5/5/10k 投影基准）。
3. **文档-实现漂移集中在「Tauri 叙事」一个根因**（**16 处字面**：ROADMAP×3+ARCHITECTURE×3+web-ui/README×4+根 README×4+api.ts 头注×1+根 Cargo.toml 注释×1——与 §2.1① 明细一致），另有 owner 元数据/edge-packaging 分叉两个独立小漂移——全部可一次 intent 闭环。
4. **推进主链只卡一个外部项**：L8 资质申请（唯一外部阻塞源头）；B1/B3/B5 与其等待期完全并行，互不阻塞。

**头号方向建议（一句话）**：B1 文档纠偏即刻开工（零依赖、半天清零 3 P1）+ B2 资质申请同步提交（外部链源头，等待期与 B3/B5 并行）——两条线互不阻塞，先跑起来。

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
| L21-22 | SQLite（bundled）元数据库：上次同步快照（三向对比基准）、状态机、pin 策略 | **已证实**（主声明） | — | bundled @ 根 `Cargo.toml:38`（`rusqlite = { version="0.37", features=["bundled"] }`）+ `crates/cloudmount-sync/src/store.rs:4` 注释；快照记录 `EntryRecord`→`entries` 表 @ `crates/cloudmount-sync/src/state.rs:95-117`；状态机 `SyncState` @ `state.rs:9`；pin 策略 `PinState`（Pinned/Online/Excluded/Inherit 往返）@ `state.rs:61-91`。**子声称漂移（小发现④，P2）**：括号注记「`水合范围位图` 属 VFS 层随 M2 落地」——源码面零证据（实跑 `grep -rni 'bitmap\|位图' crates/*/src apps/desktop/src` 零命中——排除 target 构建产物后无任何噪音，结论比「仅噪音」更强）；M2 实际以 CfAPI 内核原生检索状态+分块推送承接（`crates/cloudmount-vfs/src/backends/cfapi.rs:22,586`），前瞻注记的**命名工件未以字面落地**，能力语义已被替代方案覆盖 |
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

### §1.4 M2 · Windows 挂载（含段内 M2.5 条目，TP-07+TP-08+TP-30，12/12 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L63-73 | **CfAPI 后端基座**：SyncRoot 注册/注销幂等、占位符创建三细则、`CfConnectSyncRoot`+FETCH_DATA/CANCEL 回调（callbackcontext 上下文+FileIdentity 相对路径）、水合完成 `CF_OPERATION_TYPE_TRANSFER_DATA`（RETRIEVE_DATA 不用于完成 FETCH_DATA）、失败 `STATUS_UNSUCCESSFUL`、金标准测试 5/5（真机 cldflt，含嵌套子目录） | **已证实**（cldflt 腿本机复跑全过；历史「5/5」=当时计数，现值 8） | — | 注册幂等（先注销再注册）@ `crates/cloudmount-vfs/src/backends/cfapi.rs:284-293`；三细则（`DISABLE_ON_DEMAND_POPULATION` @ :56、`RelativeFileName` 无分隔符+父目录分组+深度升序补全中间目录 @ :397-407）；连接+回调 @ :13,:644,:734（callbackcontext/FileIdentity @ :26,:365）；**TRANSFER_DATA 选型** @ :923-934（注释明示「RETRIEVE_DATA 不用于完成 FETCH_DATA」——TP-07 证伪判据未触发）；`STATUS_UNSUCCESSFUL=0xC0000001` @ :969（失败路径 @ :846）。金标准测试 `golden_hydration_flow` @ `crates/cloudmount-vfs/tests/cfapi_hydration.rs:164`（链路=注册→占位符→连接→读触发 FETCH_DATA→水合→内容校验）+嵌套子目录 @ :227；**本机复跑**（U1 共享复跑）：cfapi_hydration 8 passed / 0 failed（11.79s 真实 cldflt 耗时）——本机=M2 开发机（plan.md 注记），真机声称升级为本机复现 |
| L74-79 | **CfapiBackend**（MountBackend 实现）：投影（远端视图→占位符）+ 水合（回调→`Provider.open_range`，downurl+Range）+ 增量投影幂等；端到端用 MockRemote 走完整链路 | **已证实** | — | `project()` 增量投影 @ `crates/cloudmount-vfs/src/backends/cfapi_mount.rs:207`（已有路径跳过=幂等 @ :225-227）；水合 `provider.open_range` @ :137,:184；MockRemote e2e @ `crates/cloudmount-vfs/tests/cfapi_backend.rs:87,131`（`backend_mounts_and_hydrates_from_provider`/`backend_project_adds_new_remote_files`，本机全过）。L78-79 的「已知边界」已由 M2.5 `update_entry` 解决（与路线图删除线叙述一致） |
| L80-82 | **PlaceholderSync 协调器**：远端视图变化→本地视图同步（新增投影 / 删除传播含嵌套子树 / 内容变化计数）；e2e：云端增删→apply→本地占位符同步+新文件可水合 | **已证实** | — | `apply_remote_view` @ `crates/cloudmount-vfs/src/backends/cfapi_mount.rs:471-527`（「新增投影、删除传播、内容变化计数」@ :471）；删除传播**嵌套子树**=逆序删除（深路径先删）@ :486-492；计数统计 `added/removed/content_updated` @ :436-440；e2e `placeholder_sync_applies_remote_add_and_delete` @ `cfapi_backend.rs:178`（本机过） |
| L83-86 | **daemon 接线完成**：`--placeholder-mount`（RemoteViewTracker：probe 短路+周期全量安全网→PlaceholderSync 应用）；跨进程全链路真机验证；`--mock` 免凭据自检通道 | **已证实**（代码面+集成测试）；「跨进程真机验证」**声称·未独立复现** | — | `--placeholder-mount` @ `crates/cloudmount-daemon/src/main.rs:62,324-325`；`RemoteViewTracker`（probe 短路返回 None @ `crates/cloudmount-sync/src/remote.rs:75`、`full_scan_every_cycles` 周期全量安全网 @ :59-68）@ `main.rs:38,744,811`；应用 @ :812；`--mock`（feature 门控）@ `main.rs:39,101-104`。跨进程真机验证（cmd/Explorer 级）属手测声称 |
| L87-94 | **M2.5 三件套**：内容变化 `CfUpdatePlaceholder`+`DEHYDRATE`（冲突保护：本地 mtime≠旧远端 mtime→跳过计冲突，绝不覆盖用户数据）；dehydrate 整文件脱水（共享冲突明确报错）；新本地目录上传 `ensure_remote_dir` 逐级创建；e2e（真机 cldflt）三场景 | **已证实**（代码面+集成测试）；真机 e2e **声称·未独立复现** | — | 冲突保护 @ `crates/cloudmount-vfs/src/backends/cfapi_mount.rs:392-424`（「本地 mtime ≠ 旧远端 mtime → 用户修改过 → 冲突，跳过」@ :408-413）；`CF_UPDATE_FLAG_DEHYDRATE`+「调用方必须先确认本地未被用户修改——DEHYDRATE 会销毁未上传的修改」@ `crates/cloudmount-vfs/src/backends/cfapi.rs:61,126-129`；共享冲突约束（独占句柄打开会失败）@ cfapi.rs:198-200；`ensure_remote_dir` 逐级 @ `cfapi_mount.rs:730-758`（视图/投影集合同步 @ :751-756）；e2e 四测试 @ `cfapi_backend.rs:239,286,554,609`+`control_dehydrate.rs:116,217,283`（本机全过）。**小发现⑤（P2）**：路线图称 API 名「`CfDehydratePlaceholder`」，实现**刻意绕开**该未文档化导出（`cfapi.rs:184-188`：实测返回成功但数据不释放、停留中间态），改用文档化两步路径（`CfSetPinState(UNPINNED)`+`CfUpdatePlaceholder`+`CF_UPDATE_FLAG_DEHYDRATE` @ :178-190）——能力语义等价，API 命名与实现不符（与 U1 小发现①③同类：协议措辞 vs 代码命名） |
| L95-97 | 踩坑：Population 策略必须 ALWAYS_FULL(3)——FULL(2) 下目录条目需提供程序信号「填充完成」才可见（dir 看不到文件+跨进程读取报「提供程序未运行」；同进程读正常） | **已证实** | — | 症状叙述逐字在位 @ `crates/cloudmount-vfs/src/backends/cfapi.rs:319-323`（「ALWAYS_FULL(3)：目录视为始终全量填充…FULL 下目录条目需提供程序信号『填充完成』才对其它进程可见（表现为 dir 看不到文件 + 跨进程读取报『提供程序未运行』）」）+ :32（模块级注记）；`CF_POPULATION_POLICY_ALWAYS_FULL` import @ :59；Shell 侧 `StorageProviderPopulationPolicy::AlwaysFull` @ `crates/cloudmount-vfs/src/backends/shell_reg.rs:80-81` |
| L98-105 | **双向写路径**：`LocalChangeUploader`（RDC 事件→三路判定→上传→`CfSetInSyncState` 绿勾）；判定表五行（Created+已投影跳过/Created+未投影上传/Modified+水合记录/远端 mtime 匹配跳过/Modified+不匹配上传/Removed 保守不上传）；「实测 Win11 内核态水合写入不产生 RDC 事件」（双 mtime 跨版本防御）；e2e 三场景全过；daemon 循环接入（先本地→远端再远端→视图） | **已证实**（代码面+集成测试）；「实测 Win11…」「e2e 真机」**声称·未独立复现** | — | 判定表（文档+实现一致）@ `crates/cloudmount-vfs/src/backends/cfapi_mount.rs:583-596`（表）+ `:624-679`（`poll_and_upload`/`handle_created`（is_projected 跳过 @ :655-659）/`handle_modified`（水合 mtime 匹配跳过 @ :665-671）/Removed 保守不上传 @ :633-637）；「双 mtime」物理形态=`hydration_log` 复用（水合记录+同步水位同表，@ :319-328「水合记录复用为同步水位」）+脱水安全检查双匹配（不匹配水合记录**也不匹配远端条目**即拒绝 @ :341-345）；`CfSetInSyncState` @ `cfapi.rs:93-116`（接线 @ `cfapi_mount.rs:722`）；daemon 循环顺序（「先处理本地→远端…再处理远端→视图」）@ `crates/cloudmount-daemon/src/main.rs:798-822`；e2e 三场景 @ `cfapi_backend.rs:341,392,444`（`bidirectional_hydration_does_not_upload`/`user_modification_uploads`/`new_file_uploads`，本机全过） |
| L106-110 | 内容变化重投影（2026-10-13）：`CfUpdatePlaceholder`+`DEHYDRATE`+冲突保护+e2e | **已证实**（与 L87-94 **同一实现载体**——`update_entry` 同函数同测试，证据共享；独立判定并在此标注重复） | — | 证据同上（`cfapi_mount.rs:392-424`+`cfapi_backend.rs:239,286`）；路线图两条目语义部分重叠属实（TP-30 行预注），计数不虚增 |
| L111-115 | **Tauri 壳**（第一处）：托盘+关窗隐藏+引擎线程+命令面（get_status/set_sync_root/toggle_pause/open_sync_folder，与 web-ui api.ts 契约对齐）+应用图标；cargo check 通过 | **漂移** | **P1** | **AC4 漂移结论**：`apps/desktop` **无 tauri 依赖**——`apps/desktop/Cargo.toml` 依赖为 `eframe 0.31 + wgpu + rfd + tray-icon + qrcode`（:13-27）+ workspace exclude（根 `Cargo.toml:12-13`，注释本身亦为漂移 D6）；`grep -rn tauri apps/desktop/Cargo.toml apps/desktop/src` 零命中。实际=egui/eframe 纯 Rust 壳（无 WebView）；托盘/关窗隐藏/引擎线程（`Arc<Mutex<SyncEngine>>`+AtomicBool）以 egui 侧等价实现在位（`apps/desktop/src/main.rs:12-14`）；「命令面 get_status/…」在桌面侧**无同名函数**（grep 零命中——egui 直连引擎非 IPC 命令层），契约对齐由 daemon `/api/*` 承载（§1.7）。与 architecture.md D1/D6 偏离基线一致；「cargo check 通过」为当时可复核声称。建议回写：改称「桌面壳（egui）」或人类裁决恢复 Tauri 路线（architecture.md 开放问题） |
| L116-123 | **应用一致性目录防护**：`exclude.rs` 四处过滤（本地扫描/RDC/inotify/远端拉取）；`.git` 纯本地+远端已有也不拉取；测试四句覆盖；静默期门控+.git 内禁冲突副本设计存档于模块文档 | **已证实** | — | 四处过滤实锚：本地扫描 @ `crates/cloudmount-sync/src/scan.rs:88`、RDC @ `crates/cloudmount-sync/src/rdc.rs:182`、inotify @ `crates/cloudmount-sync/src/inotify_watcher.rs:106`、远端拉取 @ `crates/cloudmount-sync/src/remote.rs:142`；`.git` 纯本地论证（OneDrive 结构性翻车四机制）@ `crates/cloudmount-sync/src/exclude.rs:3-29`；测试四句：`git_dir_is_invisible_to_scan` @ `scan.rs:297`、`git_dir_is_fully_ignored_by_sync_engine`（含远端 `.git` 不拉取 @ engine_e2e.rs:802）@ `crates/cloudmount-sync/tests/engine_e2e.rs:782`、`git_dir_not_projected_and_writes_not_uploaded`（占位符不投影+写入零上传）@ `cfapi_backend.rs:495`；静默期门控+.git 内禁冲突副本 @ `exclude.rs:28-29` |
| L124-128 | **Tauri 壳**（第二处，真机 GUI 手测通过）：托盘+web-ui 集成（内嵌 frontendDist，内存 23.8MB）；手测修复 custom-protocol feature、StatusSnapshot camelCase 两 bug | **漂移**（「Tauri 壳」「内嵌 frontendDist」措辞）；手测声称**声称·未独立复现** | **P1** | 同 L111-115 漂移结论（D1 同根因）：egui 壳**无 frontendDist**（web-ui 由 daemon `--serve` 托管，`crates/cloudmount-daemon/src/webdav.rs:7-14` 架构注释）；「内存 23.8MB」「手测两 bug」为真机手测声称（camelCase 契约修复在 daemon 侧 `StatusSnapshot` 序列化可佐证序列化面存在，本机测试通过）。AC4 要求的漂移结论同上，两条目一并落字 |
| L129-132 | **资源管理器状态图标与右键动词**（真机验证）：导航栏'CloudMount 云挂'；状态列'在此设备上可用'/'联机时可用'；右键原生动词'释放空间'+'始终保留在此设备上'；释放空间实测脱水成功 | **已证实**（代码面：Shell 注册+pin 状态机制齐备）；真机表现与脱水实测**声称·未独立复现** | — | Shell 侧注册（WinRT `StorageProviderSyncRootManager`，导航栏/状态图标/品牌显示）@ `crates/cloudmount-vfs/src/backends/shell_reg.rs:1-60`（DisplayName/Icon 参数面 @ :35-46）；内核侧 `CfSetPinState`/`CfSetInSyncState` @ `crates/cloudmount-vfs/src/backends/cfapi.rs:93-116`（状态列语义=pin 状态）；「CloudMount 云挂」显示名字面 @ `apps/desktop/src/main.rs:769,906,918`（壳标题/托盘 tooltip；daemon 侧经 `with_shell_integration` @ `cfapi_mount.rs:69` 传入）；右键两动词=占位符文件 OS 原生动词（无需注册，与「原生动词」措辞一致）；「释放空间实测脱水成功」=真机声称（脱水代码路径 @ `cfapi_mount.rs:343-345`+`control.rs` 转发链路在位） |

#### AC4「Tauri 壳」漂移结论（明文落字，prd AC4 要求）

> **结论：漂移（P1，两处条目 L111/L124 + L176 措辞牵连）**。`docs/ROADMAP.md` 三处「Tauri 壳」字面与实现不符：`apps/desktop` 为 **egui/eframe 0.31 + wgpu 纯 Rust 桌面壳**（无 WebView、无 tauri 依赖；Cargo.toml:13-27 + 根 Cargo.toml:12-13 exclude 注释同为漂移=D6），web-ui 已单宿主化（`platform.ts` hostKind 恒 "web"）。与 architecture.md 偏离表 **D1/D4/D6** 一致。功能面（托盘/关窗隐藏/引擎线程/状态快照/日志环形缓冲）在 egui 侧等价实现；「命令面与 api.ts 契约对齐」的承载方为 daemon `/api/*`（非桌面 IPC）。**建议回写**（U3 §2 汇总）：ROADMAP/ARCHITECTURE.md 改称「桌面壳（egui）」——或人类裁决「代码回归 Tauri」（architecture.md 开放问题，本期只审不改）。

### §1.5 M3 · Cookie 补充通道（TP-09，3/3 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L138-144 | **M3 v1**：扫码换 Cookie+只读驱动——qrcodeapi token→短轮询→passportapi 换 Cookie（真实 API 实测）；独立容器存储（物理隔离）；知情同意（`login --with-cookie-channel` 四条风险）；只读失效探测（my.115.com status 不踢线）；m115 协议加密（移植 115driver）；list/stat/open_range/quota 只读；`--channel cookie` 不等 AppID | **已证实** | — | 扫码链路 @ `crates/cloudmount-auth/src/cookie.rs:29-33`（token→qrcode→短轮询 status→login/qrcode 换 Cookie，端点 :281,:337,:362）；独立容器 `SecretKind::CookieJar`（`cloudmount/cookie-jar`）@ `crates/cloudmount-auth/src/store.rs:14-28`；知情同意**四条风险逐项** @ `crates/cloudmount-cli/src/main.rs:328-337`（①协议 3.2.3(4)/4.2(11)/5.4 ②冻结限速 ③互踢 ④无稳定性承诺；`with_cookie_channel`/`consent` @ main.rs:50-53）；只读探测 `READONLY_STATUS_PROBE="https://my.115.com/?ct=guide&ac=status"` @ `cookie.rs:47-51`（「绝不触发互踢」@ :194）；m115 移植 115driver @ `crates/cloudmount-driver115/src/m115.rs:1-3`；`--channel cookie` 路径只要求 Cookie 已启用、不要求 AppID @ `crates/cloudmount-daemon/src/main.rs:494-500`。「真实 API 实测」有历史 commit 佐证（d42456d「真实 115 端到端全链路打通」） |
| L145-149 | **M3 v2**：ec115 ECDH（P-224+AES-CBC+LZ4，移植 115driver）；initupload 秒传/sign_check/OSS 直传（STS+V1 签名+回调头）；create_dir/rename/move/copy/delete 全套写操作；Cookie 通道双向同步完整（真实端到端待扫码验证） | **已证实**（代码面）；「真实端到端待扫码验证」=路线图自声明，**现状未变**（AC5 标注：阻塞源同 M0 外部链，需真实账号扫码） | — | ec115 @ `crates/cloudmount-driver115/src/ec115.rs:5-11,20-24`（P-224 ECDH+AES-128-CBC+LZ4+固定公钥）；initupload（ECDH 加密+sign_check 区间自动计算）@ `crates/cloudmount-driver115/src/cookie.rs:489`（端点 @ :46）；OSS 直传「STS token + V1 签名 + 回调头」@ `cookie.rs:599-640`；全套写操作 @ `cookie.rs:1009,1042,1106,1120,1131,1153`（copy=异步提交+列表回查 @ :1131-1152、delete 进回收站 @ :1153-1159）——**无桩实现**（TP-09 证伪面复核过） |
| L150-153 | **自动回退**：ProviderRouter 只对认证类错误切换（频控与网络错误不切换）；切换 tracing::warn+last_switch 永不静默；`--channel auto`（默认）双通道路由 | **已证实** | — | `call` 仅对 `Err(Error::Auth(_))` 切换且一次不乒乓 @ `crates/cloudmount-driver115/src/router.rs:139-152`（「只对认证类错误切换：频控（退避重试）与网络错误（两通道同病）不切换」@ :137-138——TP-09 证伪判据「对网络/频控错误也切换」未触发）；switch 可见（last_switch 记录 @ :103-117、`tracing::warn` @ :120-131、角标数据源 @ :84-85）；`--channel auto` @ `crates/cloudmount-daemon/src/main.rs:518` |

#### M3 状态标注（AC5 要求）

- **「真实端到端待扫码验证」（L149 括号）**：路线图自带该限定且现状未变——代码面双向同步完整（上表），真实 115 账号扫码端到端**本期未执行**（外部依赖：真实账号；与 M0 L8/L16 外部链同源）。判定不因缺真实联调而降级（路线图声明本身即「待验证」措辞，诚实标注）。
- 未勾项 L154（Cookie 独有能力：分享/转存/批量操作）：未勾属实（`docs/ROADMAP.md:154`），归集见 §3（U4）。

### §1.6 M4 · 边缘网关（TP-10，2/2 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L159-164 | **WebDAV 出口+web-ui 托管**（curl 实测）：`--serve` /dav/ 走 ProviderFs（路径↔ID 解析+TTL 缓存、PUT spool 上传、MKCOL/DELETE/MOVE/COPY）；Basic 认证；web-ui 静态托管（穿越防护）；--mock 自检；4 项集成测试+curl 实测双平台通过；v1 为 HTTP | **已证实**（代码面+mock 腿集成测试现值 12/12 本机全过）；「curl 实测」**声称·未独立复现** | — | `/dav/*`→DavHandler→ProviderFs @ `crates/cloudmount-daemon/src/webdav.rs:10-11,35-37,62`；路径逐级解析+TTL 30s 缓存 @ :54-56,106-119；PUT spool 临时文件 @ :30,321-323；MKCOL（`create_dir` @ :415）/DELETE（`remove_file`/`remove_dir` @ :432,:436）/MOVE（`rename`：同父纯重命名、跨父 move+rename @ :440-461）/COPY（`copy` @ :472）；Basic 认证 @ `crates/cloudmount-daemon/src/main.rs:114`（缺省无认证注记）+负路径测试 `webdav_auth_rejects_missing_credentials`；穿越防护测试 `web_ui_traversal_vectors_rejected`（`crates/cloudmount-daemon/tests/webdav_serve.rs`）；**集成测试现值 12/12 全过**（`cargo test -p cloudmount-daemon --features mock`：webdav_serve 6+webdav_write 6——历史「4 项」=当时口径，增长含 audit-p0p1-fixes 加固面，量级对照通过；裸 `cargo test --workspace` 不含该腿=0/0，mock feature 门控与 CI `rust-mock-features` job 一致）。「curl 实测（OPTIONS/GET/PUT/回读/PROPFIND）双平台」=手测声称（现代等价=上述程序化集成测试，本机全过） |
| L165-169 | **musl 静态交叉+TLS+打包**：CI 双目标（aarch64/x86_64 musl，含 daemon+CLI+冒烟）；rustls TLS（`--tls-cert/--tls-key`，自签证书 curl 实测 HTTPS）；systemd unit（内存上限+安全加固）；Dockerfile 多架构+release workflow（GHCR）；docs/edge.md | **已证实**（静态+结构面）；「自签证书 curl 实测」**声称·未独立复现** | — | CI 双 musl 目标+冒烟 @ `.github/workflows/ci.yml:76-95`（`target: [aarch64-unknown-linux-musl, x86_64-unknown-linux-musl]` @ :81、x86_64 Smoke test @ :94-95）；rustls 参数 @ `main.rs:122-128`（`--tls-cert/--tls-key` 成对校验 @ :611）；systemd（`User=cloudmount` @ :17、`MemoryMax=64M`/`MemoryHigh=48M` @ :27-28、`NoNewPrivileges`/`ProtectSystem=strict` @ :33-34）@ `packaging/systemd/cloudmountd.service`；Dockerfile @ `packaging/docker/Dockerfile`（release.yml docker job 引用 packaging/ 侧）；release workflow GHCR：`docker` job @ `.github/workflows/release.yml:285`、`merge`（多架构清单）@ :355——九 job 名全部命中（:13,46,70,106,164,203,229,285,355）；`docs/edge.md` 存在。edge/ 与 packaging/ 双布局分叉实况=U3/TP-24 范围（architecture 分叉实证已备案） |

#### M4 验收指标状态标注（AC6 要求）

- **「512MB 内存设备稳定运行 48h」（L170 验收行）＝未验证**：无自动化观测面（无 soak/内存采样 job 或脚本佐证）；现有 `MemoryMax=64M`（packaging systemd）/`MemoryMax=256M`（edge systemd）为**配置上限**而非实测值；「局域网设备经 WebDAV 挂载读写」亦无自动化证据（手测声称）。**达成状态：未达成验证**——非外部阻塞项（不依赖 AppID/审核），属后续可开 intent 的实测任务（归集 §3，U4）。

### §1.7 M5 · 打磨（TP-11，3/3 条）

| ROADMAP 行 | 声明 | 三态 | 严重度 | 证据 |
|---|---|---|---|---|
| L174-177 | **缓存可视化/设置页/日志导出**（42ab2c9）：web-ui 完整化（标签页+占用/命中率进度条+一键清理+日志 Blob 导出，零框架 2.48KB gzip）；壳 8 命令（引擎 Arc+日志环形缓冲）；daemon serve `/api/*` REST 同名同参（同一 UI 双宿主） | **已证实**（主声明；「Tauri 壳 8 命令」子声称**漂移**——随 AC4 结论一并落字） | — | web-ui 四特性：标签导航 @ `web-ui/src/main.ts:227`、占用/命中率+百分比条 @ main.ts:36,112-113、一键清理 @ :121-134、导出日志 @ :189-194；dist 现值 gzip **2345 字节（≈2.29KB）** vs 声称 2.48KB（构建时点值微差，同量级零框架属实——`gzip -c web-ui/dist/assets/index-*.js | wc -c`）；8 命令契约 @ `web-ui/src/api.ts:44-80`（get_status/get_cache_stats/clear_cache/get_quota/export_logs/set_sync_root/toggle_pause/open_sync_folder）；daemon `/api/*` **同名同参** @ `crates/cloudmount-daemon/src/webdav.rs:925-963`（get_cache_stats/clear_cache/set_sync_root/open_sync_folder 四命令边缘模式 501=文档化边界 @ webdav.rs:940,942,960,962）；「引擎 Arc+日志环形缓冲」@ `apps/desktop/src/main.rs:12-14,55`（egui 侧等价实现）。**漂移子声称**：桌面壳无「8 命令」IPC 层（`grep -rn 'fn get_status\|…' apps/desktop/src` 零命中）——「Tauri 壳」措辞漂移同 §1.4 AC4 结论。commit 42ab2c9 存在且主题吻合（`git log -1 42ab2c9`：feat(M5) 打磨——缓存可视化/设置页/日志导出+/api REST 双宿主+万级投影基准） |
| L178-180 | **万级投影基准**（真机）：10,100 条目 618ms（16,330 条/秒）——CfCreatePlaceholders 每父目录一次内核批量调用已足够，无需优化 | **已证实**（基准测试在位且**本机复测**；历史数值=当时时点值） | — | 基准测试 `project_10k_entries_benchmark` @ `crates/cloudmount-vfs/tests/cfapi_hydration.rs:70`；**本机复测（2026-10-16，真实 cldflt）**：`万级投影基准：10100 / 10100 条目，耗时 693.8374ms（14557 条/秒）`（`cargo test -p cloudmount-vfs --test cfapi_hydration project_10k_entries_benchmark -- --nocapture`）——与声称同量级（618ms→693.8ms，测试构建档+机器状态波动）、「批量调用已足够无需优化」结论复现 |
| L182-184 | **并发水合**（0fb1c4e）：CDN 限速与 API 拆分（8 并发流）——原共用 1 r/s 限速器把多文件读取串行化；实测 4 并行下载 0.31s（修复前 ≥3s） | **已证实**（代码面）；「实测 4 并行 0.31s」**声称·未独立复现** | — | `CDN_RATE_PER_SEC=8.0`/`CDN_BURST=8.0`（8 并发流）@ `crates/cloudmount-driver115/src/client.rs:20-22`；独立 `cdn_limiter`（「勿与 API 共用——并发水合时共用会把多文件读取串行化到 1 r/s」）@ :30-32,56；CDN 直链走宽松限速 @ :174；commit 0fb1c4e 存在且主题吻合（perf(M5) 并发水合——CDN 限速与 API 拆分，实测 4 并行 0.31s） |

#### M5 状态标注（AC7 要求）

- 未勾项 L181（macOS File Provider 扩展）：未勾属实（`docs/ROADMAP.md:181`），与 `apps/shell-macos` 占位态吻合（architecture.md D3 判「一致」）——归集 §3（U4）。

### §1.8 长期 · 自研 Windows minifilter（TP-12，叙述性声明全量核验）

> 「长期」段无 checkbox（AC1 形态出入已在 §1.0 声明）；核对象=「实现方案已定稿」叙述的全部事实性声称。

| 声称 | 三态 | 证据 |
|---|---|---|
| 设计文档存在且已定稿（`drivers/win/RUST-MINIFILTER-DESIGN.md`） | **已证实** | 文件存在（46,345 字节；`ls drivers/win/RUST-MINIFILTER-DESIGN.md`）；`drivers/win/README.md` 状态「设计定稿/待实现」（4,956 字节） |
| Rust 从零实现（cdylib 直出 `.sys`，自建 FltMgr 绑定层+薄 C 壳回退路径） | **已证实** | cdylib 直出 `.sys` @ design.md:453（KM-0 行）；回退路径（「内核侧保留 <800 行 C（仅 DriverEntry+回调转调），逻辑仍全在 Rust…必须在 KM-1 前保持可切」）@ :95 |
| 14 项稳定机制 | **已证实** | 「## 7. 十四项稳定机制」表 @ design.md:284（第 1–14 行逐项，:289-:301） |
| S1–S6 门禁 | **已证实** | S1（卸载零泄漏）…S6（兼容性矩阵）表 @ design.md:31-38（「每条机制都映射到 S1–S6」@ :38） |
| KM-0…KM-7 里程碑 | **已证实** | KM-0…KM-7 @ design.md:21,:453（KM-0 骨架行含门禁/工期） |
| 18–30 人月（验证占半） | **已证实** | 「18–30 人月（含验证）…验证占一半以上」@ design.md:21 |
| 官方两条红线已写入设计 | **已证实** | 红线①（HSM 组内不得对 `RECALL_ON_DATA_ACCESS` 文件做 targeted 缓存读写）+②（必须处理 section 同步钩子）@ design.md:19,:66（展开论述 @ :65-68） |
| 零售分发需过 HLK/WHCP（2026 签名政策收紧） | **已证实** | design.md:20（「零售分发需 HLK/WHCP…Server 2016+ 不接受 attestation」）+:402（「HLK 认证当作独立里程碑（KM-7）」） |
| 「不阻塞任何产品里程碑」 | **已证实** | design.md:462（「M2 用 CfAPI 发布产品；cmflt 完成后经 MountBackend 切换…不阻塞任何产品里程碑」） |

---

## §2 计划-实现偏离（TP-15..20+TP-24，U3）

> 格式=四栏记录：**文档/路线图字面｜实际实现｜证据｜建议回写位置**。偏离基线=architecture.md D1–D6+苗头对账注记（r1）；本节为「复核+收口+扩展扫描」，不重复 §1 已展开的判定细节。

### §2.1 四苗头分项对账（AC8，TP-15/18/19/20）

| # | 苗头 | 结论 | 文档/路线图字面 | 实际实现 | 证据 | 建议回写位置 |
|---|---|---|---|---|---|---|
| ① | 「Tauri 壳」vs egui | **为真（漂移，P1）** | ROADMAP L111/L124「Tauri 壳」两 `[x]` 条目+L176「Tauri 壳 8 命令」；ARCHITECTURE.md:22/:48/:86-87；web-ui/README.md:7,10,18,23；web-ui/src/api.ts:2-4 头注；**根 README.md:89,96,133,151（扩展扫描新增，见 §2.2 NEW-1）**；根 Cargo.toml:12 注释 | `apps/desktop`=egui/eframe 0.31+wgpu+rfd+tray-icon 纯 Rust 壳，无 tauri 依赖无 WebView（Cargo.toml:13-27）；`apps/desktop/src-tauri/` 不存在 | `grep -rn -i tauri apps/desktop/Cargo.toml apps/desktop/src`→零命中；§1.4 AC4 漂移结论专节（三态判定已落字） | ROADMAP 三处+ARCHITECTURE.md 三处+web-ui/README.md+api.ts 头注+根 README 四处+根 Cargo.toml:12 注释——统一改称「桌面壳（egui/eframe）」；或人类裁决代码回归 Tauri（architecture.md 开放问题） |
| ② | `apps/shell-macos` 占位 | **非偏离（一致，TP-19）** | ROADMAP L181 `[ ]` macOS File Provider 未勾 | `apps/shell-macos/` 仅 README.md（无代码）；README.md:3「状态：**占位**。计划于 Windows 版（M2）稳定后启动」+「目录（待建）」节 | `ls apps/shell-macos`→仅 README.md；README.md:3,:25 | 无需纠偏（README 与路线图未勾吻合=D3）；启动 M5 时按 README 方案（staticlib+Swift 薄桥）建目录 |
| ③ | `.gitignore`/`gen_icons.py` src-tauri 残留 | **半真半假（分项判定）** | 苗头声称两处残留 | `.gitignore:13-14` 残留**为真**（`apps/desktop/src-tauri/gen/`、`apps/desktop/src-tauri/target/` 两行）；`scripts/gen_icons.py` 残留**为假**（`grep -i tauri scripts/gen_icons.py` 零命中——desktop-installers intent 已清除） | `.gitignore:13-14`；gen_icons 零命中；守护断言在位 @ `scripts/desktop-installers-yaml.test.sh:697-698`（TP-11 负断言「gen_icons.py 无 src-tauri 字面」） | `.gitignore:13-14` 两行删除（待后续 intent，审核期不动）——归 U4 §3 backlog 归集 |
| ④ | web-ui 角色变迁未回写 | **为真（漂移，P1）** | ROADMAP L114「与 web-ui api.ts 契约对齐」/L159-162 web-ui 托管/L174 web-ui 完整化——以「双宿主」语境叙述；ARCHITECTURE.md §前端复用「一套 SPA 两种宿主」 | `platform.ts` 已单宿主化（:2-5 头注「桌面端已切换为纯 Rust（egui + wgpu + rfd，apps/desktop），不再使用本 web-ui。本模块保留 fetch 单路径」，hostKind 恒 "web"）；唯一宿主=`cloudmountd --serve`（webdav.rs:7-14） | `web-ui/src/platform.ts:2-5`；api.ts 契约仍为两宿主措辞（:2-4「桌面 Tauri 壳与边缘 cloudmountd 都要实现」） | web-ui/README.md+api.ts 头注改「边缘单宿主」；ROADMAP L114 的「契约对齐」改指 daemon `/api/*`（webdav.rs:925-963）；ARCHITECTURE §前端复用重写（=苗头①同批回写） |

### §2.2 扩展扫描新增偏离（AC9，TP-24；D1–D6 之外）

| # | 结论 | 文档字面 | 实际 | 证据 | 建议回写位置 |
|---|---|---|---|---|---|
| NEW-1 | **根 README.md 四处 stale Tauri（P1，D 表锚面之外的新增）** | README.md:89「桌面壳（Tauri 2）」、:96 架构图「Tauri 桌面壳」、:133「apps/desktop Tauri 2 桌面壳」、**:151 `cd apps/desktop && cargo tauri dev`（不可执行死命令——apps/desktop 无 tauri 依赖与 tauri.conf.json，`apps/desktop/src-tauri/` 不存在）** | egui 壳（同苗头①） | `grep -rn -i tauri README.md`→:89,:96,:133,:151；`ls apps/desktop`→无 src-tauri/tauri.conf.json | README.md 四处改 egui 表述、:151 命令改 `cargo run`；**首屏文档漂移，建议随苗头①同批回写并优先** |
| NEW-2 | **webdav.rs 模块注释「TLS 见 M4 后续」已过时（P2）** | webdav.rs:29「HTTP（无 TLS）——LAN 场景；TLS（rustls + 证书管理）见 M4 后续」 | M4 已落地 rustls TLS（`--tls-cert/--tls-key` @ daemon main.rs:122-128,611——serve 层包装） | 两处对照 | webdav.rs:29 注释更新为「TLS 可选（--tls-cert/--tls-key，见 main.rs serve 装配）」 |
| NEW-3 | **ROADMAP L164「v1 为 HTTP（TLS 见后续）」与 L167 同里程碑内「rustls TLS 已落地」表述未收敛（P2）** | 同一 M4 段相邻两条目先称「TLS 见后续」再称「rustls TLS（--tls-cert/--tls-key，自签证书 curl 实测 HTTPS）」 | TLS 可选非默认已实现 | ROADMAP.md:164 vs :167 | 收敛口径见 §4/AC13⑦（一句话版本）；ROADMAP L164 括号改「（TLS 可选，见下条）」 |
| NEW-4 | **edge/ 与 packaging/ 部署面已实际分叉（P2，登记 architecture 分叉实证+本次复证）** | edge/Dockerfile:5 宣称 `linux/amd64,linux/arm64,linux/arm/v7` 三平台、edge/systemd 内联 `serve --listen 127.0.0.1:8384` 无 EnvironmentFile、MemoryMax=256M | packaging/Dockerfile:4 仅 `linux/amd64,linux/arm64`；packaging/systemd 走 `EnvironmentFile=-/etc/default/cloudmountd`+`$LM_ARGS`、MemoryMax=64M；release.yml docker matrix 实际仅 `linux/amd64`/`linux/arm64`（:295-297）且引用 packaging/ 侧 | 两 systemd 逐行对照（edge:14,19 vs packaging:20,22,27）；两 Dockerfile 头注；release.yml:293-297 | 收敛主从需人类裁决（architecture.md 开放问题）；短期先在 edge/Dockerfile 头注声明「CI 发布面以 packaging/ 为准，armv7 未提供」 |
| NEW-5 | **owner 元数据不一致（P2，=D5 复核+错侧判定）** | 根 Cargo.toml:20 `repository=zhonghaoxi/cloudmount` | `apps/desktop/Cargo.toml:7`=`Tintac-CN/cloudmount`；git remote=`git@github.com:Tintac-CN/cloudmount.git`；release.yml 镜像 `ghcr.io/${GITHUB_REPOSITORY_OWNER}/...`（:307，实际=Tintac-CN）——**根 Cargo.toml 为错侧** | 三方对照 | 根 Cargo.toml:20 改 `Tintac-CN/cloudmount`（installer-packages backlog 已备案，随下个 intent 落） |

### §2.3 记录性口径（非偏离，TP-24/TP-13 附注）

- **断言脚本计数环境相关**：`desktop-installers-yaml.test.sh` 本机复跑 **112/112**（actionlint 可用环境为 113——缺 actionlint 时 yaml.safe_load 降级路径少 1 断言；历史亦见 111）；其余四脚本 72/23/13/9 与环境无关。TP-13 判据取「与环境一致的末行计数+exit 0」，满足。
- **workspace crate 清单一致性**：根 Cargo.toml:3-11 members 七 crate 与实际 `crates/` 目录一致、`exclude=["apps/desktop"]` @ :13——无漂移。
- **除上表外，本档扩展扫描未发现其它新增偏离**（AC9 显式声明）：ROADMAP↔drivers/win 设计文档字面（TP-12 已逐项对应）、根 Cargo.toml workspace 结构、五脚本计数口径均已核对一致。

---

## §3 归集表（TP-21/22/23，U4）

### §3.1 路线图开放项归集（AC10，TP-23）

> ROADMAP 未勾项全量 7 处（`grep -n '\- \[ \]' docs/ROADMAP.md`：L8/L16/L35/L36/L133/L154/L181）+ 长期段叙述性开放项 + 本期审核新增候选。阻塞性质：**外部**=依赖第三方（115 审核/GitHub/Apple），**内部**=本仓可自行开工。

| 开放项 | 来源锚 | 依赖源 | 阻塞性质 | 可独立开 intent 最小粒度 | 证据位置 |
|---|---|---|---|---|---|
| 申请 115 开放平台个人开发者+创建 PKCE 应用 | ROADMAP.md:8（**外部阻塞链源头**，:3-4「唯一的外部阻塞项」，审核约 7+7 工作日） | 115 开放平台人工审核 | **外部** | 单独 intent：材料提交+受理跟进（人类操作为主，工程侧零代码） | §1.1 M0 未勾项表 |
| 待 AppID 实测：真实账号跑通全链路；确认 copy/新建目录响应字段 | ROADMAP.md:16 | 上项出 AppID | 外部（间接） | 单独 intent：真实凭据全链路实测+`open.rs:274,371` 两处 `TODO(verify)` 闭环 | §1.1；`crates/cloudmount-driver115/src/open.rs:274,371` |
| 待真实凭据联调：两台设备经 115 完成一次干净双向同步 | ROADMAP.md:35 | 上项（依赖 M0 的 AppID，字面自证） | 外部（间接） | 并入上项 intent（同一真实凭据前置，拆分反增成本） | §1.1 |
| 验收指标：空闲 RSS ≤ 20MB（`cloudmountd --once` 下观测） | ROADMAP.md:36 | 真机测量（本机即可，无外部依赖） | 内部 | 小 intent：观测脚本+数据留痕（可与 AppID 实测 intent 合并执行） | §1.1 |
| MSIX 打包（可选：MSI+NSIS 已覆盖，仅 Store 渠道需要） | ROADMAP.md:133 | Store 渠道决策（人类偏好） | 内部（可长期搁置） | 独立 intent，仅当 Store 分发被裁决需要时开 | §1.4 未勾标注 |
| Cookie 独有能力：分享/转存/批量操作（二级入口+常驻风险提示） | ROADMAP.md:154 | M3 通道已完成（无外部依赖）；UI 入口裁决 | 内部 | 单独 intent：三能力+二级入口+风险常驻提示 | §1.5 M3 状态标注 |
| macOS File Provider 扩展（Swift 薄桥+core staticlib） | ROADMAP.md:181 | `apps/shell-macos` 占位（README 方案在案）；付费 Apple Developer 账号；mac 真机；**core 导出 ABI 冻结策略裁决**（architecture.md 开放问题） | 外部（账号/真机）+内部（ABI 裁决） | PoC intent（staticlib+薄桥最小挂载）→ 扩展 intent（两段式） | §1.8；`apps/shell-macos/README.md:3-25` |
| 自研 Windows minifilter（长期） | ROADMAP.md:186-194（无 checkbox） | 设计已定稿（RUST-MINIFILTER-DESIGN.md，KM-0…KM-7，18–30 人月） | 内部（长周期，**不阻塞任何产品里程碑**） | 按 KM-0 起分段 intent（每 KM 一个） | §1.8 |
| **【本期新增】M4 验收实测：512MB 内存设备稳定运行 48h+局域网 WebDAV 真机读写** | §1.6 状态标注（ROADMAP.md:170 验收行） | 一台低内存设备/受控环境 | 内部 | 小 intent：soak 观测+真机读写留痕 | §1.6；§4.2 |
| **【本期新增】文档纠偏批：stale Tauri 全量面+owner 元数据+.gitignore 死路径+TLS 注释** | §2.1①④+§2.2 NEW-1..3/NEW-5（P1×3+P2 文档项） | 无（零依赖） | 内部 | 单独 intent：纯文档/元数据回写（约 15 处，见 §5 B1） | §2.1-§2.2；§6/TP-18,24 |
| **【本期新增】部署面主从收敛裁决（edge/ vs packaging/）** | §2.2 NEW-4 | 人类裁决主从（architecture.md 开放问题） | 内部（裁决后开工） | 裁决→小 intent（声明废弃方或合并） | §2.2 NEW-4 |
| **【本期新增】Intel mac 无 desktop 交付补登** | §4.2 边界 3（dmg aarch64-only「已知取舍」未列入任何开放项） | 人类确认是否需要 x86_64 dmg | 内部 | 若需要：小 intent（dmg job 加 x86_64 target）；若不需要：仅文档声明 | §4.2#3；`release.yml:200` |

### §3.2 四 intent backlog 归集（AC11，TP-21+TP-22 去重）

> 输入面=四 `ops-runbook.md`「P2 backlog」+「观察项」节 + 四 `retro.md`「下个 bolt 要改的」（过程学习节不入，AC11 约束）。去重标记：**〔D:n源〕**=跨 runbook 重复已合并。分类：代码/文档/CI/发布。

| # | 归集条目 | 来源 intent（含行锚） | 去重 | 分类 | 关联开放项/备注 |
|---|---|---|---|---|---|
| 1 | ci.yml 增 actionlint 步骤（语法拦截前移） | macos-release(runbook:29)+installer-packages(runbook:32)+desktop-installers(沿用，runbook:33) | **〔D:3源〕** | CI | 消 §4.1 缺口 3；retro 三处同诉（macos-release retro:31、installer retro:32、desktop retro:30） |
| 2 | Cargo.toml repository 元数据修正（zhonghaoxi→tintac-cn） | installer-packages(runbook:38)+desktop-installers(沿用，runbook:33) | **〔D:2源〕** | 文档 | =NEW-5/D5（根 Cargo.toml:20 错侧，§2.2） |
| 3 | check-macos.yml `permissions:` 显式化（最小权限加固） | macos-release(runbook:30) | — | CI | architecture.md CI/CD 权限节佐证（release.yml 六 build job 同类缺口可一并考虑） |
| 4 | 断言脚本 YAML 解析精度（job_block 行启发式→真解析） | macos-release(runbook:31) | — | CI | — |
| 5 | package-macos.sh tar 半成品清理（临时名成功后 mv） | macos-release(runbook:32) | — | 发布 | — |
| 6 | `.gitattributes` 行尾统一（release.yml CRLF/check-macos.yml LF） | macos-release(runbook:33) | — | CI | retro 优先项（macos-release retro:31） |
| 7 | state.json U2 计数口径勘误注记（「MSI 侧 27 条」为时点值） | installer-packages(runbook:33)+desktop-installers(沿用) | **〔D:2源〕** | 文档 | 记录性 |
| 8 | decisions「移除 ICE80」措辞勘误注记 | installer-packages(runbook:34) | — | 文档 | 记录性 |
| 9 | T15 注入自校验正分支在 actionlint 可用环境补跑留痕 | installer-packages(runbook:35) | — | CI | 与条目 1 联动（actionlint 可用后自然可做） |
| 10 | .wxs 无 UIRef 知会（双击安装走缺省 basic UI） | installer-packages(runbook:36) | — | 发布 | 知会性质 |
| 11 | release 下载步注释「只取 binaries」语义漂移修正 | installer-packages(runbook:37) | — | 文档 | — |
| 12 | wxs Package Description/Comments 两件套描述（ARP 元数据欠准） | desktop-installers(runbook:32 备案①) | — | 发布 | 需新授权（既有字面守护在案） |
| 13 | HKCU KeyPath 多用户卸载孤儿（ICE43 经典代价） | desktop-installers(runbook:32 备案②) | — | 发布 | 决策留痕，记录性 |
| 14 | dmg 脚本不校验二进制架构 | desktop-installers(runbook:32 备案③) | — | 发布 | — |
| 15 | TP-23 无显式承接载体（建议补埋点） | desktop-installers(runbook:32 备案④) | — | 测试 | — |
| 16 | T4 修订断言①防空真依赖步骤名字面（结构脆弱性） | desktop-installers(runbook:32 备案⑤) | — | CI | — |
| 17 | desktop 断言计数环境相关口径（112/113）注记进 README/脚本头注 | desktop-installers(runbook:32 备案⑥) | — | 文档 | §2.3 已复证该口径 |
| 18 | gen_icons 幂等跨 zlib 版本边界（注记） | desktop-installers(runbook:32 备案⑦) | — | 文档 | — |
| 19 | TP-12 build.rs 'winres' 子串宽松（现态无害） | desktop-installers(runbook:32 备案⑧) | — | 测试 | 记录性 |
| 20 | FFI GetWindowLongPtrW 32 位约束注释留痕 | desktop-installers(runbook:32 备案⑨) | — | 文档 | 记录性 |
| 21 | units/2.md 契约字面补伪隐藏偏差指针 | desktop-installers(runbook:32 备案⑩) | — | 文档 | 已完结 intent 工件——按「不改原始工件」原则经新 intent 处理或豁免 |
| 22 | .gitignore L13-14 src-tauri 死路径残留清理 | desktop-installers(runbook:32 备案⑪) | — | 文档 | =D2/§2.1③（TP-15 复证为真半边）；并入 §5 B1 |
| 23 | **CI 补 desktop fmt/clippy 步骤**（终验口径缺口流水线上游补死） | desktop-installers(**retro:31——唯一未沉淀进 runbook 的新可执行候选**) | — | CI | §4.1 缺口 1/2 的上游消解；体积小 |
| 24 | hash.rs:157 per-run 唯一 temp 目录（根治间歇 flaky 头号嫌疑） | audit-p0p1-fixes(runbook:54+retro:31 优先序) | — | 代码 | 31+5 跑零复现的 flaky 观察（runbook:54） |
| 25 | exec.rs 错误路径 dir_creating 条目残留 | audit-p0p1-fixes(runbook:55) | — | 代码 | — |
| 26 | webdav.rs:513 冗余条件 | audit-p0p1-fixes(runbook:55) | — | 代码 | — |
| 27 | cookie_mock/mock_api ~60 行平行实现 | audit-p0p1-fixes(runbook:55) | — | 代码（重构） | — |
| 28 | cookie.rs:584 initupload 解密明文日志收口为 status/pickcode | audit-p0p1-fixes(runbook:55+retro:31 优先序) | — | 代码（安全卫生） | F-04 脱敏不变量延伸 |
| 29 | 观察项：macos-15 runner 退役公告跟踪（退役时三源同步改 runs-on+断言） | macos-release(runbook:24)+installer-packages(runbook:27)+desktop-installers(runbook:28) | **〔D:3源〕** | CI | 与 §4.2 边界 5 联动 |
| 30 | 观察项：WiX 3.14.1.20250415 废弃公告跟踪（升 v4/v5 需 dotnet SDK 重写 .wxs） | installer-packages(runbook:26)+desktop-installers(runbook:27) | **〔D:2源〕** | 发布 | 与 §4.2 边界 4 联动 |
| 31 | 观察项：安装包/dmg/托盘真机核验触发（首个含安装包 tag 后 T17-T21/T14-T16/release-checklist 人工清单） | installer-packages(runbook:28)+desktop-installers(runbook:18 特别项 c/d+runbook:26)+macos-release(retro:32) | **〔D:3源〕** | 发布 | 与 §5 B4 真机实测 intent 天然合流 |
| 32 | 观察项：间歇 flaky CI 现身时按失败测试名定位 | audit-p0p1-fixes(runbook:54) | — | 测试 | 条目 24 的运行时伴生观察 |
| 33 | 环境备注：`cargo test -p cloudmount-sync`（无 features）编译失败为既有状态（需 `--features testutil`） | audit-p0p1-fixes(runbook:56) | — | 测试 | test-strategy M9 口径一致；可作条目 24/27 同批小修（加门控） |
| 34 | retro 候选：CI release workflow tag 三平台构建打包自动化 | audit-p0p1-fixes(retro:32) | — | — | **已被后续 intent 落地消解**（release.yml 九 job 现状，§4.1/TP-16）——标注，不再排队 |
| 35 | retro 候选：伪隐藏 mac 形态二选一（Dock 留存 vs LSUIElement）随真机实测裁决 | desktop-installers(retro:32+runbook:26) | — | 代码 | 并入条目 31 真机核验批 |
| 36 | retro 候选：gen_icons 扩尺寸集沿 stdlib 方案扩（幂等边界注记随行） | desktop-installers(retro:33) | — | 代码 | 条件性（仅当需要 256/512 组） |
| 37 | retro 候选：AskUserQuestion 超时降级为对话式一句话裁决 | macos-release(retro:33)+installer-packages(retro:34) | **〔D:2源〕** | — | 编排流程习惯（非本仓工程项），记录不入工程队列 |

### §3.3 归集完整性与去重自检（TP-21/TP-22 判据落点）

- **覆盖**：四 runbook P2 共 4+5+7+11+沿用 3=30 条原始条目 → 去重合并后 **28 条**（条目 1-28，其中条目 34 系 retro 落地消解不占工程队列）+ 观察项 5 组（29-33，三组多源已并）+ retro 新增候选 4 条（34-37）——逐条可溯源至 `docs/aidlc/<intent>/ops-runbook.md:行` 或 `retro.md:行`（表内已标）。
- **去重正确性**（TP-22）：六组重复（actionlint 3 源/owner 元数据 2 源/decisions 勘误 2 源/macos-15 观察 3 源/WiX 观察 2 源/真机核验 3 源）均合并单行并标 **〔D:n源〕**；无不同项误并（actionlint=CI 语法门禁 vs 断言脚本解析精度=条目 4，分列未混）。
- **计数不虚高**：43 条 ROADMAP `[x]` 声明与 backlog 零重叠（已完成项不入队列）；「~4 项+环境备注」口径与实读 4 P2+flaky 观察+环境备注吻合（audit-p0p1-fixes runbook:54-56）。

---

## §4 质量与交付面盘点（TP-13/14/16+AC12/AC13，U3）

### §4.1 测试与 CI 现状（AC12）

**基线复跑（TP-14，2026-10-16 本机）**：

| 项 | 命令 | 结果 | 对照 |
|---|---|---|---|
| workspace 测试 | `cargo test --workspace`（U1 共享复跑） | **277 passed / 0 failed**（26 目标） | desktop runbook「277/0」逐数一致；≠M1 历史口径 114（§1.2 TP-03 双口径注记） |
| workspace fmt | `cargo fmt --all -- --check` | **exit 0（clean）** | runbook 口径一致 |
| desktop fmt | `cd apps/desktop && cargo fmt --all -- --check` | **exit 0（clean）** | desktop-installers r1 全量 fmt 回填后无债（2a02050），历史「desktop fmt 债」已清 |
| workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 一致 |
| desktop clippy | `cd apps/desktop && cargo clippy --all-targets -- -D warnings` | **exit 0** | 一致（根级命令不触 exclude 成员——retro 教训，双面分开跑为必要口径） |
| 五断言脚本（TP-13） | `bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh` | **112/112、72/72、23/23、13/13、9/9 全 exit 0** | desktop runbook 硬门「五脚本全绿 112/72/23/13/9」逐数一致（计数环境相关注记见 §2.3） |

**三 CI 工作流结构（TP-16，声称↔实锚逐项命中）**：

| 工作流 | 声称 job | 实锚行 | 触发 |
|---|---|---|---|
| ci.yml | `rust`/`rust-mock-features`/`web-ui`/`arm-cross` 四 job | :14/:39/:59/:75 | `push: branches [main]`+`pull_request`（:4-6） |
| check-macos.yml | `check-macos` 单 job | :11 | **仅 `workflow_dispatch`**（:5，无 push 触发） |
| release.yml | 九 job（binaries-linux/windows/macos、installer-msi-windows、installer-pkg-macos、installer-dmg-macos、release、docker、**merge**） | :13/:46/:70/:106/:164/:203/:229/:285/:355 | **仅 `push: tags ["v*"]`**（:4-5） |

**CI 覆盖缺口清单（AC12 要求，逐条带证据）**：

1. **根级 fmt/clippy/test 对 `apps/desktop` 零效力**：workspace `exclude=["apps/desktop"]`（根 Cargo.toml:13）——desktop 必须单独跑（本审核 TP-14 即双面口径）；ci.yml 四 job 均不触 apps/desktop。
2. **desktop 4 测试 CI 不可见**：`grep -rc '#\[test\]\|#\[tokio::test\]' apps/desktop/src/` = 4；无任何 workflow job 覆盖 desktop（仅 release.yml 打包构建非测试）——desktop 回归靠本机手跑。
3. **三工作流均无 actionlint 步骤**：`grep -rn actionlint .github/workflows/` 零命中——workflow YAML 语法回归无门禁（五断言脚本中的 yaml 断言为静态替代面；actionlint 可用环境 113 差额即源于此）。
4. （非缺口佐证）webdav mock 集成腿 CI 有覆盖：ci.yml `rust-mock-features` job（:39）三 OS `--features cloudmount-daemon/mock`——U2 补跑的 12 项 webdav 测试在 CI linux/win/mac 腿执行。
5. （非缺口佐证）check-macos 仅手动触发=刻意低频（check 腿与 ci.yml arm-cross/release mac 腿互补，非缺覆盖）。

### §4.2 已知边界七项盘点（AC13，逐项三栏——**本节=TP-27 执行载体**〔AC13 主载体，gate1 r1 新增 TP；第 1 项主证=TP-06/§1.3，第 7 项含 AC6 TLS 口径收敛〕）

| # | 边界 | 当前状态 | 残留风险 | 是否被后续里程碑消解 |
|---|---|---|---|---|
| 1 | M1「同秒内等大小修改漏检」 | **已关闭**（M1.5 内容校验通道默认开启：`verify_every_cycles=5`+每轮 32 文件轮转 @ engine.rs:65-70,87-90；盲区回归+关闭行为双测试在位 @ engine_e2e.rs:631,675——§1.3 L49-51 主证） | 用户显式关闭校验时盲区回归——**文档化行为**（e2e `content_verification_can_be_disabled` 固定该语义 @ :675,:698） | **已被 M1.5 消解**（默认路径不再漏检） |
| 2 | mac 产物无 Developer ID 签名/公证（arm64 链接器 ad-hoc）；Windows MSI 无 Authenticode | **属实**：release body 三行明示（mac dmg 行 @ release.yml:263「未签名」、:265「mac 产物无 Developer ID 签名/公证（arm64…ad-hoc）」、:266「Windows MSI 无 Authenticode」、:267「mac pkg 无…」）——流水线无任何签名步（grep codesign/sign 仅有 ad-hoc 说明） | 首启 Gatekeeper/SmartScreen 拦截（body 附解除指引：右键→打开/`xattr -cr`/更多信息→仍要运行）；企业分发不可用 | **未消解**（无后续里程碑声称签名；需付费 Apple Developer 账号+EV 证书——外部依赖，macos-release/installer runbook backlog 域） |
| 3 | dmg aarch64-only | **属实**：`installer-dmg-macos` 仅 `--target aarch64-apple-darwin`（release.yml:210-211「aarch64 原生」）+「aarch64-only…Intel mac 无 desktop 交付为已知取舍」注释 @ :200 + body「aarch64-only」@ :263 | Intel mac 无 desktop 渠道（可用 `cloudmount-darwin-aarch64` CLI tar.gz 非桌面壳） | **未消解**（显式取舍，未列入任何开放项——如需 x86_64 dmg 应归集为新 backlog 项） |
| 4 | WiX 硬锁 3.14.1.20250415 | **属实**：`choco install wixtoolset --version 3.14.1.20250415` 硬锁+candle 缺失即 `exit 1`（release.yml:145-147「版本 3.14.1.20250415 硬锁」） | WiX v3 处于维护模式；锁死版本免疫上游破坏但累积升级债（v3→v4 迁移为大改） | **未消解**（installer-packages backlog 域；属刻意锁定非疏漏） |
| 5 | `macos-15` runner 依赖 | **属实**：check-macos.yml:13+release.yml:72,166,205 四处 `runs-on: macos-15`（「pin macos-15（arm64）单 runner 双 target」裁决注释 @ :79,check-macos.yml:20） | GitHub 淘汰旧 runner 镜像时 CI 突然失败；arm64-only runner 决定 mac 产物架构面（与边界 3 联动） | **未消解**（gate1 Q1 刻意 pin；升级 runner=有意识变更） |
| 6 | Cookie 通道条款风险 | **属实且已有防护**：协议条款引用 @ cloudmount-auth/src/lib.rs:5-9（3.2.3(4)/4.2(11)/5.4+挤掉其它设备会话）；CLI 知情同意**四条风险逐项**+默认关闭+`yes` 显式确认 @ cli main.rs:328-352；独立容器物理隔离（store.rs:14-28）；切换常驻提示（router.rs:120-131「补充通道有协议风险，UI 应常驻提示」） | 账号冻结/限速/协议随时变更失效（风险②④）；无官方追索 | **未消解**（设计上以「官方通道优先+Cookie 兜底+显式知情同意」控制；该通道本身即 M3 交付物，风险为其已知属性——归集 §3 的 L154 未勾项延续） |
| 7 | WebDAV TLS 口径收敛（AC13「无内置 TLS/无速率限制配置」vs M4「rustls 可选」） | **收敛为一句**：WebDAV v1 **默认 HTTP（LAN 场景），TLS 为可选非默认**——`--tls-cert/--tls-key` 成对传入即 rustls HTTPS（main.rs:122-128,611；「自签证书 curl 实测」为手测声称·未独立复现）；速率限制**无用户配置面**（API 1 r/s+CDN 8 r/s 内置拆分 @ client.rs:20-22,55-56，CLI/daemon 无限速参数）。AC13 原表述「无内置 TLS」按此句理解；「TLS 见后续」类过时叙述=§2.2 NEW-2/NEW-3 | 缺省 HTTP 明文+Basic 认证缺省关闭（main.rs:114「缺省无认证，仅建议内网」）——公网直暴露风险由部署文档对冲（docs/edge.md 安全节+Tailscale 建议） | **已被 M4 消解**（TLS 可选能力落地；「默认关」为设计取向非缺口） |

### §4.3 审核自身可信度自检（TP-17）

> 方法：从 §1 已写发现中抽 3 条，独立重跑其 §6 命令，比对输出与报告引用（含退出码/行号/行文本）。

| 自检项 | 报告引用（§6 锚） | 独立重跑输出 | 比对 |
|---|---|---|---|
| U1 §1.1 L9（Commands::Login 注册行号） | `main.rs:47,114` | `grep -n 'Login' crates/cloudmount-cli/src/main.rs` → `47: Login {`、`114: Commands::Login {` | **一致**（行号零漂移） |
| U2 §1.4 L95-97（ALWAYS_FULL(3) 症状叙述行号） | `cfapi.rs:319-323` | `grep -n 'ALWAYS_FULL(3)\|提供程序未运行' …/cfapi.rs` → `319:`、`321:` | **一致**（行文本逐字命中） |
| U1 §1.3 L43-45（RDC 5/5 本机复跑） | `5 passed; 0 failed` | `cargo test -p cloudmount-sync --lib rdc` → `running 5 tests`+`test result: ok. 5 passed; 0 failed`（0.46s） | **一致**（可原样重跑） |

**结论：抽样 3/3 可复现，报告证据链可信（TP-17 通过；非抽样发现的其余条目均由同口径命令支撑，重跑路径已在 §6 逐条给出）。**

---

## §5 下一阶段路线图（TP-25/26，U4）

> **排序依据（TP-26 三据=依赖/风险/价值）**+与 ROADMAP「里程碑依赖」图自洽（ROADMAP.md:196-201：M0→M1→M2→M3→M5，M2→M4 可并行）。**不发明需求**（TP-25）：每批次均回溯到 ROADMAP 未勾项/验收行或 §3 归集条目（出处列内标注）；未列能力不入队。

### §5.1 排序批次表（AC14 固定六列）

| 批次 | 方向（出处） | 依赖 | 风险 | 价值 | 建议 intent 边界 |
|---|---|---|---|---|---|
| **B1** | **文档与元数据纠偏批**（出处：§2.1①④+§2.2 NEW-1..3/NEW-5+§3.2 条目 2/22）——README 四处 stale Tauri（含 `cargo tauri dev` 死命令）+ARCHITECTURE 三处+ROADMAP 三处+web-ui README/api.ts 头注+根 Cargo.toml repository+`.gitignore:13-14`+webdav.rs:29 注释+脱水 API 命名注记 | **无（零依赖，可立即开工）** | 低（纯文档/元数据，无行为变更；唯一注意=与「代码回归 Tauri」裁决互斥——若人类选代码回归则本批改向，裁决点=architecture.md 开放问题） | 高（P1 漂移×3 清零；首屏 README 死命令消除；路线图可信度修复的最后一公里；成本约半天） | 单一 intent「docs-backfill」：一次闭环约 15 处回写+五断言脚本守护断言补位（防回潮）；不碰行为代码 |
| **B2** | **M0 外部阻塞链启动**（出处：ROADMAP.md:8+§3.1 首行）——提交 115 开放平台个人开发者申请+创建 PKCE 应用材料 | 无（人类操作为主） | **外部审核 7+7 工作日不可控**（唯一外部阻塞项——排序论证：启动越早解锁越早；等待期不阻塞 B1/B3/B5 并行） | 极高（解锁 B4 AppID 实测→L16/L35 闭环→真实凭据全链路） | 单独轻量 intent：材料清单+提交+受理跟进留痕（工程侧零代码） |
| **B3** | **backlog 批量清理：CI 加固+测试卫生**（出处：§3.2 条目 1/3/6/9/16/23/24/25/26/28/33 等）——actionlint 步骤〔D:3源〕、desktop fmt/clippy CI 步骤（retro 唯一新候选）、check-macos permissions 显式化、hash.rs flaky 根治、cookie.rs:584 日志收口、exec dir_creating 残留、行尾统一等 | 无（独立可并行） | 低-中（分散小项多，需防 scope 膨胀——建议按「CI 加固/测试卫生」两小批或一个 intent 内分区验收） | 中-高（CI 语法门禁从无到有；flaky 根治保基线可信；日志脱敏不变量延伸；全为既有备案非新需求） | 一个 intent「backlog-sweep」分区验收（CI 区+代码卫生区），或拆两个小 intent；观察项类（29/30/32）不入本批（跟踪性质） |
| **B4** | **M0 AppID 实测+真实凭据联调**（出处：ROADMAP.md:16,:35,:36+§3.1）——真实账号全链路+copy/create_dir 响应字段确认+两台设备双向同步+RSS ≤20MB 观测 | **B2 审核通过出 AppID**；真机/真账号 | 中（实测可能暴露 `TODO(verify)` 处响应字段偏差——open.rs:274,371 预留了该预期；RSS 指标若不达标需调优） | 极高（M0 两条未勾验收闭环；ROADMAP M0 段收口；后续真实环境置信度基础） | 一个 intent「m0-realworld」：实测+字段确认+TODO 闭环+RSS 观测留痕；**与 B6 的真机核验观察项（§3.2 条目 31）合流执行** |
| **B5** | **M3 Cookie 独有能力**（出处：ROADMAP.md:154+§3.1）——分享/转存/批量操作+二级入口+常驻风险提示 | M3 通道已完成（无外部依赖）；UI 入口形态裁决（轻） | 中（条款风险为通道已知属性——沿用知情同意+常驻提示设计；批次本身无新增合规面） | 高（Cookie 通道差异化能力；M3 段最后一项） | 一个 intent「cookie-extras」：三能力+二级入口+风险常驻提示（router last_switch 角标既有设计承接） |
| **B6** | **M4 边缘验收实测+部署面收敛**（出处：§1.6 状态标注+§3.1 新增项+NEW-4）——512MB/48h soak+局域网 WebDAV 真机读写；edge/packaging 主从收敛（裁决先行） | 低内存设备/受控环境；**人类裁决 edge-packaging 主从** | 中（48h 长周期观测；指标若不达标需性能调优=范围扩张风险——实测 intent 应限定「测并留痕」） | 中（M4 验收行闭环；部署文档可信度） | 一个 intent「m4-soak」：soak 脚本+观测留痕+真机读写；收敛裁决单列（可先出裁决问卷） |
| **B7** | **M5 macOS File Provider**（出处：ROADMAP.md:181+§3.1）——staticlib+Swift 薄桥 PoC→扩展 | **付费 Apple Developer 账号+mac 真机（外部）**；core ABI 冻结策略裁决（architecture.md 开放问题）；shell-macos README 方案（在案） | 高（外部依赖两项+新平台工程量大；ABI 未冻结则薄桥反复破坏——architecture.md 已预警） | 高（mac 平台占位符体验=M5 压轴项；ROADMAP M5 段收口） | 两段式：先「abi-freeze+PoC」小 intent（staticlib 导出面+最小挂载验证），再「provider-ext」主体 intent |
| **B8** | **MSIX 打包**（出处：ROADMAP.md:133） | Store 渠道裁决（人类偏好） | 低（可选路径，MSI+NSIS 已覆盖 GitHub 分发） | 低-中（仅 Store 渠道有价值） | 条件 intent：仅当 Store 分发被裁决需要时开；否则保持未勾并注明 |
| **长期** | **自研 minifilter**（出处：ROADMAP.md:186-194）——按 KM-0 起分段 | 设计定稿（在案）；无产品依赖 | 高（18–30 人月，验证占半；内核开发固有风险） | 高（长期旗舰）但**不阻塞任何产品里程碑**（设计文档 :462 明示） | 每 KM 一个 intent（KM-0 骨架起步）；门禁 S1–S6 为准出硬条件 |

### §5.2 批次间依赖关系

```text
B1（文档纠偏）───────────────────────────→ 随时可开工（含与 B2 等待期并行）
B2（资质申请）──→ [外部审核 7+7 工作日] ──→ B4（AppID 实测+联调+RSS）
B3（backlog 清理）──────────────────────→ 独立并行；其 CI 产物为 B4-B7 提供门禁
B5（Cookie 独有能力）──────────────────→ 独立（M3 已闭环）
B6（M4 实测+收敛）←── 前置裁决：edge-packaging 主从（人类）
B7（macOS File Provider）←── 外部（账号/真机）+ 裁决（core ABI 冻结）
B8（MSIX）←── 裁决（Store 渠道）
长期 minifilter ── 独立轨道（不阻塞任何产品里程碑）
```

- **与里程碑依赖图自洽**（ROADMAP.md:196-201）：M0→M1→M2→M3→M5 主链——M1/M2/M3 的 `[x]` 已核（§1），故图中剩余推进点=M0 实测（B4）、M3 最后一项（B5）、M5 压轴（B7）；M4 支线（B6）与 M2+ 可并行——本排序未绕开任何未完成前置（TP-26 证伪判据「绕开 M0 先做 M5」未触发：B7 的启动前置是外部账号与裁决，其工程 PoC 若提前开工也不消费 M0 未验项）。
- **头号方向建议（一句话，§0 预览）**：**B1 文档纠偏即刻开工（零依赖、半天清零 3 P1）+ B2 资质申请同步提交（外部链源头，等待期与 B3/B5 并行）——两条线互不阻塞，先跑起来**。

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

### TP-07（M2·CfAPI 基座与 CfapiBackend，L63/L74）

- `grep -n 'CfConnectSyncRoot\|CF_OPERATION_TYPE_TRANSFER_DATA\|RETRIEVE_DATA\|STATUS_UNSUCCESSFUL' crates/cloudmount-vfs/src/backends/cfapi*.rs` → `cfapi.rs:734`（连接）、`:923-934`（TRANSFER_DATA+「RETRIEVE_DATA 不用于完成 FETCH_DATA」）、`:846,:969`（STATUS_UNSUCCESSFUL=0xC0000001）。证伪判据（TRANSFER_DATA 缺失或用 RETRIEVE_DATA）未触发。
- 三细则 → `cfapi.rs:56`（DISABLE_ON_DEMAND_POPULATION）、`:397-407`（RelativeFileName 无分隔符/父目录分组/深度升序）；幂等注册 → `:284-293`；callbackcontext/FileIdentity → `:26,:365,:644`。
- 金标准测试存在性 → `ls crates/cloudmount-vfs/tests/`（cfapi_backend.rs 13 测试/cfapi_hydration.rs 8 测试/control_dehydrate.rs 3 测试）；`golden_hydration_flow` @ `cfapi_hydration.rs:164`（注释「金标准链路」@ :3）。本机复跑（U1 共享 `cargo test --workspace`）：cfapi_hydration **8 passed / 0 failed（11.79s）**＝cldflt 腿真实执行（本机=开发机）。
- project 幂等 → `cfapi_mount.rs:207,225-227`（`projected.contains(rel) → continue`）；open_range 水合 → `:137,:184`。

### TP-08（M2·daemon 接线与双向写路径，L83/L98）

- `grep -rn 'LocalChangeUploader\|CfSetInSyncState\|RemoteViewTracker\|placeholder-mount' crates/` → `cfapi_mount.rs:596`（struct）+判定表 `:583-596`+实现 `:624-679`（is_projected 跳过/水合 mtime 跳过/Removed 保守）；`cfapi.rs:93-116`（set_in_sync）+`cfapi_mount.rs:722`（接线）；daemon `main.rs:38,62,324-325,801,811-812`（tracker/uploader/循环）；`--mock` @ `main.rs:39,101-104`。
- 判定表五行与实现逐行对照 → 文档表 `cfapi_mount.rs:585-593`（Created 双分支/Modified 双分支/Removed=不上传）；双 mtime 防御物理形态 → `:319-328`（hydration_log 复用为同步水位）+`:341-345`（脱水安全双匹配拒绝）。
- 循环顺序 → `main.rs:798-822`（注释「先处理本地→远端…再处理远端→视图」）；probe 短路+周期全量安全网 → `crates/cloudmount-sync/src/remote.rs:52-68,75`。
- 真机声称（跨进程验证/Win11 内核态水合无 RDC 事件）→ **声称·未独立复现**（无自动化载体；判定表与 e2e 的程序化等价面本机全过）。

### TP-30（M2·剩余六条批量，L80/L87/L95/L106/L116/L129）

- L80：`apply_remote_view` @ `cfapi_mount.rs:471-527`（逆序删除嵌套子树 @ :486-492、计数 @ :436-440）；e2e `cfapi_backend.rs:178`。
- L87/L106：冲突保护 @ `cfapi_mount.rs:392-424`；DEHYDRATE 字面 @ `cfapi.rs:61,126-129`；共享冲突约束 @ `cfapi.rs:198-200`；ensure_remote_dir @ `cfapi_mount.rs:730-758`；e2e @ `cfapi_backend.rs:239,286,554,609`+`control_dehydrate.rs:116,217,283`。L106 与 L87 同载体——标注重复。
- L95：`cfapi.rs:319-323`（ALWAYS_FULL(3)+FULL(2) 症状叙述逐字）+`:32,:59`；`shell_reg.rs:80-81`。
- L116：四处过滤 → `scan.rs:88`/`rdc.rs:182`/`inotify_watcher.rs:106`/`remote.rs:142`；`.git` 论证 @ `exclude.rs:3-29`（禁冲突副本+静默期门控 @ :28-29）；测试四句 → `scan.rs:297`+`engine_e2e.rs:782,802`+`cfapi_backend.rs:495`。
- L129：`shell_reg.rs:1-60`（WinRT 注册参数面）；pin 状态 @ `cfapi.rs:93-116`；「CloudMount 云挂」@ `apps/desktop/src/main.rs:769,906,918`。真机表现/脱水实测 → **声称·未独立复现**。

### TP-09（M3 全量，L138/L145/L150）

- 扫码链路 → `crates/cloudmount-auth/src/cookie.rs:29-33,281,337,362`；独立容器 → `store.rs:14-28`；四条风险 → `crates/cloudmount-cli/src/main.rs:328-337`（逐项四条）+参数 `:50-53`；只读探测 → `cookie.rs:47-51,194-199`。
- ec115 → `crates/cloudmount-driver115/src/ec115.rs:5-11,20-24`；initupload/STS/V1/回调 → `cookie.rs:46-48,489,599-640`；全套写操作 → `cookie.rs:1009-1159`（copy 异步回查/delete 进回收站，无桩）。
- Router → `crates/cloudmount-driver115/src/router.rs:103-131`（switch 可见）、`:137-152`（**仅 `Err(Error::Auth)` 切换**、一次不乒乓——证伪判据未触发）；`--channel auto/cookie` → `crates/cloudmount-daemon/src/main.rs:494-500,518`。
- 状态标注：L149「真实端到端待扫码验证」现状未变（外部依赖：真实账号扫码）；历史佐证 commit d42456d（真实 115 端到端全链路打通）。

### TP-10（M4 全量，L159/L165）

- WebDAV → `crates/cloudmount-daemon/src/webdav.rs:10-11,54-56,106-119,284,321-323,365,415,432,436,440-472`（/dav/→ProviderFs/TTL 30s/PUT spool/五方法）；Basic 认证 → `main.rs:114`；穿越防护+认证负路径测试在 webdav_serve.rs。
- mock 腿集成测试：`cargo test -p cloudmount-daemon --features mock --test webdav_serve --test webdav_write` → **6+6=12 passed / 0 failed**（裸 workspace 0/0=feature 门控，与 ci.yml `rust-mock-features` 一致）；历史「4 项」=当时口径。
- musl/TLS/systemd/docker/edge.md → `ci.yml:76-95`；`main.rs:122-128,611`（--tls-cert/--tls-key）；`packaging/systemd/cloudmountd.service:17,27-28,33-34`；`packaging/docker/Dockerfile`；`ls docs/edge.md` 存在；release.yml 九 job 名 @ `:13,46,70,106,164,203,229,285,355`（docker @ :285、merge @ :355）。
- 验收指标「512MB/48h」→ **未验证**（无观测面；MemoryMax 为配置上限非实测）。

### TP-11（M5 全量，L174/L178/L182）

- web-ui 8 命令契约 → `web-ui/src/api.ts:44-80`（恰 8 个）；daemon 同名同参 → `webdav.rs:925-963`（8 命令，4 个边缘 501 边界 @ :940,942,960,962）；web-ui 完整化 → `main.ts:36,112-113,121-134,189-194,227`；gzip 现值 → `gzip -c web-ui/dist/assets/index-*.js | wc -c` = 2345（≈2.29KB，声称 2.48KB 为时点值）；egui 侧 Arc/环形缓冲 → `apps/desktop/src/main.rs:12-14,55`；「壳 8 命令」IPC 层 → grep 零命中（漂移子声称，随 AC4 结论）。
- 万级投影基准 → `cfapi_hydration.rs:70`（测试）；**本机复测** `cargo test -p cloudmount-vfs --test cfapi_hydration project_10k_entries_benchmark -- --nocapture` → `万级投影基准：10100 / 10100 条目，耗时 693.8374ms（14557 条/秒）`（与声称 618ms/16,330 同量级，结论复现）。
- 并发水合 → `client.rs:20-22,30-32,56,174`（CDN 8.0 r/s 拆分）；commit 存在性 → `git log -1 42ab2c9`（feat(M5) 打磨…）、`git log -1 0fb1c4e`（perf(M5) 并发水合——8 并发流）。
- 真机声称（4 并行 0.31s/内存 23.8MB）→ **声称·未独立复现**（0.31s 有 commit message 留痕；无自动化基准）。

### TP-12（长期段叙述性声明全量）

- `ls drivers/win/RUST-MINIFILTER-DESIGN.md` → 存在（46,345 字节）；结构逐项 grep → cdylib/.sys ×8 命中（KM-0 行 @ :453）、薄 C 壳回退 @ :95、14 项 @ :284（表 :289-:301）、S1–S6 @ :31-38、KM-0…KM-7 @ :21,453、18–30 人月 @ :21、两条红线 @ :19,66、HLK/WHCP @ :20,402、「不阻塞任何产品里程碑」@ :462。

### TP-13（假完成探测器·五断言脚本计数复跑）

- 命令：`bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`（末行+退出码逐一采集）。
- 输出摘要：`112/112 passed`、`72/72 passed`、`23/23 passed`、`13/13 passed`、`9/9 passed`，**五脚本全 exit 0**——与 desktop runbook 硬门「112/72/23/13/9」逐数一致；环境相关口径（actionlint 可用 113）见 §2.3。证伪判据（任一 N<声称或退出非零）未触发。

### TP-14（假完成探测器·基线测试与 lint 现值）

- `cargo test --workspace` → 277/0（U1 共享复跑，同机同日）；`cargo fmt --all -- --check` → exit 0；`cd apps/desktop && cargo fmt --all -- --check` → exit 0；`cargo clippy --workspace --all-targets -- -D warnings` → exit 0；`cd apps/desktop && cargo clippy --all-targets -- -D warnings` → exit 0。证伪判据（desktop 侧未覆盖却被叙述为全绿）未触发——本报告以双面分开跑为口径。

### TP-15（假完成探测器·路径引用死链，负路径分项对账）

- ① `.gitignore` L13-14 `apps/desktop/src-tauri/{gen,target}/` 残留 → `grep -rn 'src-tauri' .gitignore scripts/` → `.gitignore:13,14` 两行命中（**发现候选=D2 属实**）；scripts/ 面仅断言脚本守护行命中（desktop-installers-yaml.test.sh:697-698）。
- ② `scripts/gen_icons.py` src-tauri 引用 → `grep -i tauri scripts/gen_icons.py` **零命中**（苗头半边前提已失效=「为假」，desktop-installers 已修复）。
- ③ ROADMAP 声称路径全量 `test -e`（29 条路径清单含 drivers/win/RUST-MINIFILTER-DESIGN.md、web-ui/src/api.ts、docs/edge.md、packaging/{systemd,docker}、edge/*、crates 六模块文件等）→ **零 MISS**。

### TP-16（假完成探测器·CI job 名与触发条件存在性）

- `grep -nE '^\s{2}[a-z-]+:' .github/workflows/ci.yml` → `rust`(:14)/`rust-mock-features`(:39)/`web-ui`(:59)/`arm-cross`(:75)；触发 → `push: branches [main]`+`pull_request`（:4-6）。
- check-macos.yml → `check-macos`(:11)+`on: workflow_dispatch`(:5，无 push)；release.yml → 九 job（U2 已核 :13…:355，含 `merge`）+`on: push: tags ["v*"]`（:4-5）。证伪判据（job 名不存在或触发不符）未触发。

### TP-17（审核自身可信度·抽样独立重跑）

- 抽样 3 条（§1.1 L9 行号锚/§1.4 L95-97 行文本锚/§1.3 L43-45 命令锚）独立重跑 → 三者输出与报告引用逐字一致（行号零漂移、命令可原样重跑）——明细见 §4.3 表。证伪判据（一次性交互操作/重跑不符/行号漂移）未触发。

### TP-18（偏离扫描·Tauri↔egui 字面漂移，2/12 分区标注）

- 字面全量面 → `grep -rn -i tauri docs/ROADMAP.md docs/ARCHITECTURE.md Cargo.toml apps/desktop/Cargo.toml README.md` → ROADMAP:111,124,176（2/12 条目=L111/L124，判定在 §1.4；L176 为 M5 条目内子声称）+ARCHITECTURE:22,48,86-87+根 Cargo.toml:12+**README:89,96,133,151（扩展扫描新增=NEW-1）**；实现面 → `grep -rn -i tauri apps/desktop/Cargo.toml apps/desktop/src` 零命中+`ls apps/desktop`（无 src-tauri/tauri.conf.json）。收口见 §2.1①+§1.4 AC4 结论（不重复展开）。

### TP-19（偏离扫描·shell-macos 占位）

- `ls apps/shell-macos` → 仅 `README.md`；README.md:3「状态：**占位**。计划于 Windows 版（M2）稳定后启动」+:25「目录（待建）」→ 与 ROADMAP L181 未勾**一致，非偏离**（§2.1②）。证伪判据（报告称已实现或忽略占位态）未触发。

### TP-20（偏离扫描·web-ui 角色变迁回写）

- `platform.ts:2-5` 头注（单宿主化自证「桌面端已切换为纯 Rust…不再使用本 web-ui」）vs `api.ts:2-4` 头注（仍双宿主措辞「桌面 Tauri 壳与边缘 cloudmountd 都要实现」）+web-ui/README.md:7,10,18,23 → **角色变迁未回写=真**（§2.1④）；ROADMAP 对 web-ui 的「M4 边缘专属」定位未反映 daemon `/api/*` 双宿主现状（webdav.rs:925-963 实锚）。

### TP-24（文档互相一致性/偏离扩展扫描）

- D5 owner 元数据 → 根 Cargo.toml:20 `zhonghaoxi/cloudmount` vs apps/desktop/Cargo.toml:7 `Tintac-CN/cloudmount` vs `git remote -v`=`Tintac-CN/cloudmount.git` vs release.yml:307 `${GITHUB_REPOSITORY_OWNER}`（实际 Tintac-CN）→ **根为错侧**（NEW-5）。
- edge/packaging 分叉复证 → edge/systemd:14,19（内联 listen+256M）vs packaging/systemd:20,22,27（EnvironmentFile+$LM_ARGS+64M）；edge/Dockerfile:5 三平台宣称 vs packaging/Dockerfile:4 双平台+release.yml:293-297 matrix `linux/amd64`/`linux/arm64`（NEW-4）。
- workspace 结构 → 根 Cargo.toml:3-13 members 七 crate+exclude 一致；crate 清单 vs `crates/` 目录一致。
- stale Tauri 全量面三锚+README 四处 → 见 TP-18；新发现 NEW-1..5 全录 §2.2；除 NEW-1..5 外未发现其它新增偏离（§2.3 显式声明）。

### TP-21（归集完整性·四 runbook+retro 逐条对照——引用 §3.2 既有证据，不重执行）

- 证据主体=§3.2 表 37 行（每行「来源 intent（含行锚）」列即溯源锚）；原始输入面实测锚：audit-p0p1-fixes runbook:52-56（P2 四项 @ :55、flaky 观察 @ :54、环境备注 @ :56）、macos-release runbook:27-33（P2 五项 @ :29-33）、installer-packages runbook:30-38（P2 七项 @ :32-38）、desktop-installers runbook:30-33（备案十一项 @ :32+沿用三项 @ :33）；retro「下个 bolt」：audit-p0p1 retro:31-34、macos-release retro:31-33、installer-packages retro:32-34、desktop-installers retro:30-33（实文件 `grep -n` 实测——gate2 M4 订正后口径）。
- 「过程学习节不入」约束执行：四 retro 的过程学习节（什么有效/没用/学到）未产生归集行；retro 候选仅 4 行入表（条目 23/34/35/36/37 中 34=已消解标注、37=非工程项）。

### TP-22（归集去重正确——引用 §3.2 去重标记+§3.3，不重执行）

- 证据主体=§3.2 六组 **〔D:n源〕** 标记（条目 1〔D:3源〕/2〔D:2源〕/7〔D:2源〕/29〔D:3源〕/30〔D:2源〕/31〔D:3源〕）+§3.3 去重算术（30 条 P2 原始−4=26 行；11 条观察−5=6 行；retro 独有 5 行=37）。
- 反例核对（无误并）：actionlint 门禁（条目 1）与断言脚本解析精度（条目 4）分列；真机核验（条目 31）与 LSUIElement 形态裁决（条目 35）分列。

### TP-23（开放项集合完整性——引用 §3.1 既有证据，不重执行）

- 证据主体=§3.1 表 12 行（ROADMAP 未勾 7 处全覆盖：L8 外部阻塞链源头标注入表 @ 首行、L16/L35/L36/L133/L154/L181 各行+长期 minifilter 行+本期新增 4 行）。
- 抽取命令（可重跑）：`grep -n '\- \[ \]' docs/ROADMAP.md` → L8/L16/L35/L36/L133/L154/L181 七行——与 §3.1 前七行一一对应，无缺漏（尤其 L8 在列）。

### TP-25（排序「不发明需求」守护——引用 §5.1 出处列，不重执行）

- 证据主体=§5.1 批次表「方向（出处）」列：B1←§2.1①④+§2.2 NEW-1..3/NEW-5+§3.2 条目 2/22；B2←ROADMAP.md:8；B3←§3.2 条目 1/3/6/9/16/23/24/25/26/28/33；B4←ROADMAP.md:16,35,36；B5←ROADMAP.md:154；B6←§1.6 状态标注+NEW-4；B7←ROADMAP.md:181；B8←ROADMAP.md:133；长期←ROADMAP.md:186-194——每项均回溯到 ROADMAP 未勾项/验收行或 §3 归集条目，零新发明能力。

### TP-26（排序依据三据充分性——引用 §5.1/§5.2，不重执行）

- 证据主体=§5.1 六列表（每行含依赖/风险/价值三据，B2 行含「外部阻塞链源头优先」论证：唯一外部项、7+7 工作日、启动越早解锁越早）+§5.2 依赖图与 ROADMAP.md:196-201 里程碑依赖自洽说明（未绕开未完成前置）。

### TP-27（AC13 已知边界七项批量——执行载体=§4.2，不重执行）

- 证据主体=§4.2 七项三栏表（本节即其执行载体）：①同秒漏检→引 TP-06 结论（§1.3 L49-51，不重复执行——TP-27 行预注）；②未签名声明→release.yml body :263-267 字面+无签名步；③dmg aarch64-only→release.yml:200,210-211+body :263；④WiX 硬锁→release.yml:145-147；⑤macos-15 runner→check-macos.yml:13+release.yml:72,166,205 四处；⑥Cookie 条款→auth/lib.rs:5-9+cli 四条风险（§1.5 交叉）；⑦TLS 口径收敛→§4.2 第 7 项「收敛为一句」+§2.2 NEW-2/NEW-3。每条含当前状态/残留风险/是否消解三栏结论——TP-27 通过判据全满足。

---

*（U4 收尾：TP-01..30 全执行——§0 摘要/§3 归集 37 条/§5 路线图 8+1 批次落盘，AC15 五项自检全过（§7），报告完成待 gate2。审核零写入承诺全程保持：本文件为唯一仓库写入物。）*

---

## §7 报告完整性自检（AC15 五项机械判据，U4 收口执行）

> 判据来源=prd AC15：「同时满足以下五项才判『报告完成』，任一不满足即『报告未完成』」。以下为逐项机械核查记录（2026-10-16，U4 收口时点）。

**① §0–§6 七章齐备且非空 —— 通过。**
`grep -cE '^## §[0-6]'` = **7**（§0 摘要/§1 断言核验/§2 偏离/§3 归集/§4 体检/§5 路线图/§6 证据索引，另有 §7 本自检节）；各章均有实质内容（空占位已全部替换）。

**② AC1–AC13 每条 AC 在 §1–§4 有可按 AC 编号定位的落点 —— 通过。**（AC14/AC15 落 §5/§7）

| AC | 落点 | AC | 落点 |
|---|---|---|---|
| AC1 | §1.0（方法与判定口径基线，含长期段形态出入声明） | AC8 | §2.1（四苗头四栏记录） |
| AC2 | §1.1（M0 七条表+两未勾项标注表） | AC9 | §2.2+§2.3（NEW-1..5+「无其它新增偏离」显式声明） |
| AC3 | §1.2+§1.3（M1/M1.5 表+TP-03 对照小节+盲区关闭结论 @ §1.3 L49-51 行） | AC10 | §3.1（开放项 12 行） |
| AC4 | §1.4（M2 12 行表+「AC4 Tauri 壳漂移结论」专节落字） | AC11 | §3.2+§3.3（37 条归集+去重自检） |
| AC5 | §1.5（M3 表+「M3 状态标注」节） | AC12 | §4.1（现状+CI 缺口清单 3 条实锚） |
| AC6 | §1.6（M4 表+「M4 验收指标状态标注」节） | AC13 | §4.2（七项三栏，含 TLS 收敛句） |
| AC7 | §1.7（M5 表+「M5 状态标注」节） | AC14 | §5.1/§5.2（六列批次表+依赖图） |
| — | — | AC15 | §7（本节） |

**③ 每条非「已证实」判定与每条 P0/P1 发现均有 §6 可解析证据行 —— 通过。**
非「已证实」判定共 2 条（L111/L124 漂移）→ §6/TP-18（grep tauri 命令+命中行清单：ROADMAP:111,124,176/ARCHITECTURE:22,48,86-87/Cargo.toml:12/README:89,96,133,151+`ls apps/desktop` 反证）。P0=0；P1 共 3 条（L111、L124、NEW-1）→ L111/L124 同 TP-18 证据行；NEW-1 → §6/TP-18+TP-24（README 四行锚+`ls apps/desktop` 无 src-tauri）。P2 九条均已在 §1/§2 行内带 `文件:行` 证据（AC15③ 对 P2 无强制 §6 行要求，事实覆盖）。

**④ §5 含排序批次表与批次间依赖 —— 通过。**
§5.1=固定六列（批次/方向/依赖/风险/价值/建议 intent 边界）9 行（B1-B8+长期）；§5.2=依赖图+里程碑依赖图自洽说明；每批次方向栏内含出处（ROADMAP 行锚/§2/§3 条目号——TP-25「不发明需求」守护）。

**⑤ §0 计数矩阵与各章明细行数一致 —— 通过（机械复核）。**
§1 声明行=7+9+7+12+3+2+3=**43**（§1.1 另含未勾项标注表 2 行，不计入 43）；§1.8=**9** 行；漂移=**2**（L111/L124）→ 已证实 41；§3.1=**12** 行；§3.2=**37** 行；§5.1=8 批次+1 长期；P1=3/P2=9/P0=0——与 §0.2 矩阵逐格一致（awk 按节计数复核，见上方执行记录）。

**结论：AC15 五项全部通过 → 报告完成（gate2 可按 prd AC15 机械判定）。**
