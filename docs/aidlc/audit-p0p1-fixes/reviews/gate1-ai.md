# Gate#1 AI 互审（2026-10-14）

> 评审对象：prd.md / architecture.md / test-strategy.md @ 3e0046c/3ca71c1/7635b2d
> 评审者：reviewer-pm（1/7/7）、reviewer-architect（0/4/6）、reviewer-qa（1/6/9）、reviewer-dev（0/5/7）——只读交叉评审，全部对照仓库源码逐项核实。
> 编排者去重合并：**2 BLOCKER / 11 MAJOR / 18 MINOR**。基线结论：F-01…F-14 缺陷现状与行号全部属实，架构两处审计修正（service.rs 仅 FFI 入口死、config::load_from 语义）成立；问题集中在**测试规格与验收口径**，非方向性错误。

## BLOCKER

- **B1**［pm B-1 + qa M-5］**AC15 双重悬空**。①"内存基准（空闲 RSS 5-15MB）不回退"无判据对象：全仓无 benches/、无 RSS 测量代码，"5-15MB"仅是 main.rs:26 设计纪律注释，未定义平台/静置状态/测量工具/阈值；②AC15 无任何测试点映射，且 CI 矩阵语义（windows-only 自动跳过、mock feature 未开启）无人看守。修复方向：RSS 子句改人工清单判据（静置 RSS ≤15MB 流程）或删子句引用 AC1 分块断言；补 T-25（CI 矩阵 + features + skip 语义），使每个 AC 有映射落点。
- **B2**［qa B-1 + dev M1］**T-05（P0 安全主测试点）照单执行=全绿零覆盖**。①攻击向量全部带 `/dav/` 前缀，但 webdav.rs:804-812 把 `/dav/*` 路由给 ProviderFs（另一套未审计路径解析），到不了被修复的 serve_static（:922-932）；②现有 harness webdav_serve.rs:30 以 `web_ui_dir=None` 启动，非 /dav 路径直接 404（:819-823），被测函数不可达；③hyper `Uri` 对 `\` 等字符需百分号编码，否则 parse panic。修复方向：向量去前缀打静态路由面 + harness 传 web-ui fixture（含根外金丝雀）+ 向量编码化。

## MAJOR

- **MA1**［arch MAJOR-1 + dev M2］**F-10 单飞不彻底，40140117 病灶残留**。设计只改 `access_token()`，而 `Authenticator::refresh()`（open.rs:366-374，被 client.rs:244 在并发重试路径调用）仍是"拿 rt→丢锁→网络→persist"，并发双刷互踩正是 F-10 要消灭的形态；AC10/T-17/T-18 只测 access_token()。修复：refresh() 复用同一持锁刷新助手；补 refresh() 并发测试。
- **MA2**［pm M-2 + arch MAJOR-2 + dev M5］**desktop 壳绕过 Cookie 开关，范围裁决缺失**。apps/desktop/src/main.rs:143-149 硬编码 CookiePolicy{true,true} 且 GUI 无同意交互、不写 [cookie] 段；与 U5"产品合规边界在**所有**组装路径一致生效"、安全不变量 #4 直接冲突；架构一边标"construction 检查项"一边把 apps/desktop 划出触及面，自相矛盾。需 Gate 裁决：纳入修复 / 后续 intent + 修正措辞 / 记录残留。
- **MA3**［pm M-4/M-5 + arch MAJOR-3 + qa M-2/m-4/m-5 + dev M3］**F-14/F-04/F-11 测试注入面两文档互斥 + 可达性缺口**。①arch 定 `#[cfg(test)]` 私有字段注入（"不改公开 API"）vs strategy 定 tests/ 集成 + `with_base_urls` 扩参——集成测试够不到私有字段，二者只能居其一；②`oss_put` 是私有方法（cookie.rs:542），经公开 upload() 到达它需先构造 ECDH 加密的 initupload 成功响应（开放问题 #5 未决；dev 指出 p224/aes/lz4_flex 已是正式依赖、无新供应链）；③T-24 要求 initupload 成功响应与 AC14 已回填基线矛盾，且与 mock_api.rs:225-263 既有 Open 通道秒传测试重复，归属须写明；④cookie.rs:605 `format!("https://{bucket}.{OSS_ENDPOINT}/…")` 硬编码 scheme，注入字段须存完整基址或改写格式串。需 Gate 裁决（ECDH 回环纳入与否）+ 统一两文档。
- **MA4**［arch MAJOR-4］**F-06 Drop 有界 join 的前提未证实**。设计依赖"CloseHandle 唤醒另一线程在途的 ReadDirectoryChangesW"——官方文档未将关句柄列为受支持取消手段，对在途同步 I/O 关句柄属未定义行为（网络盘尤甚，本项目监听本地目录风险低但非零）；新设计 join 后失败模式从"泄漏"变"Drop 永久挂起"。修复：join 超时（超时 detach + warn 降级）或辅以 CancelSynchronousIo，或 Gate 显式接受该风险。
- **MA5**［qa M-1］**测试点映射列未回填 AC 编号**。T-01…T-24 列头"映射 AC"值全为 F-xx（1:1 可机械映射、无虚构），Construction 消费前必须回填为 AC-x ↔ F-xx 双锚，否则 unitStatus 覆盖追溯断链。
- **MA6**［qa M-3 + pm m-3］**两处断言机制要求的生产侧 seam 未在架构声明**：T-09 依赖 rdc.rs 内置 AtomicUsize 活跃计数 hook（架构 F-06 设计未含）；T-01 单元腿要求分块拷贝循环提为 crate 内可测函数（架构把循环内联在水合闭包）。补入设计。
- **MA7**［qa M-4］**T-15（P0）注入机制与真实错误源错位**。F-09 的 Err 产生于 resolve_path→**远端 Provider** 调用（webdav.rs:291-309），strategy 给的 share-none 本地句柄/权限注入压不住该路径；harness MockRemote 无错误注入能力。需 error-injectable 的 Provider 包装（`Arc<dyn Provider>` 直接可实现，改动小）。
- **MA8**［dev M4 + qa M-6 + dev m4］**CI 门控语义缺口，"三平台绿"对安全项空绿**。webdav_serve.rs 整文件 `#![cfg(feature = "mock")]` 而 CI 只跑 `cargo test --workspace`（无 --features）→ F-03/F-08/F-09 集成测试 CI 从不执行；cfapi 集成腿（T-01/T-02）在 GH windows runner 因 SyncRoot 注册失败（0x80070490）自动跳过。修复：CI 增加带 `--features cloudmount-daemon/mock` 的 job；AC15/T-25 写明 skip 面与 windows-only 清单。
- **MA9**［pm M-1］**--interval 潜伏 bug 无验收落点**。架构 F-12 已纳入"build_launch_args 补 --interval"（源码证实：install 不烘焙、service_main 却解析、服务模式恒默认 30s），但 PRD 无 AC、T-21 无 build_launch_args 快照断言。回填。
- **MA10**［pm M-6］**AC12 死代码判据空转**。service.rs:148 现有 `#[allow(dead_code)]` 压制告警——"dead_code 无告警"判据今天就是绿的，无判别力。改结构性断言（`define_windows_service!`/`service_main` 不复存在）或声明"删除 allow 属性后以告警为判据"。
- **MA11**［pm M-3/M-7 + arch MINOR-4］**F-05 auto 通道与开放问题清单缺口**。auto 时"静默跳过 Cookie 兜底"的行为变化无 AC 断言、T-08 未区分 channel 取值；PRD 开放问题缺 AC14 裁决点与 F-05 升级行为/同意存储 A/B（现仅 architecture 开放问题 #2 持有）。AC5 补 auto 断言；PM 开放问题补全。

## MINOR

- **m1**［arch MIN-1］AC1/T-01 断言口径钉死在 reader 级读取序列（read_to_end 几何增长 vs 1MiB 循环可区分、可造红测）；勿按 open_range 的 RangeSpec 断言（修复前后都只调一次，无法区分）。
- **m2**［arch MIN-2 + qa m-1］F-04 捕获机制修正：MakeWriter 属 tracing-subscriber 而 driver115 零 dev-deps → 手写 Subscriber（约 40 行，仅用 tracing 本体）或修订决策 #1；捕获级别统一（AC4 trace 全开 vs T-07 debug）。
- **m3**［arch MIN-3］F-08 spool 清理语义三方漂移（arch=warn 后接受残留 / T-14=重试最终成功 / AC8="延迟清理"含糊）+ flushed 置位点未定义（若上传尝试即置位会掩盖二次 flush 失败）。统一口径并修 T-14/AC8；明确 flushed 在上传确认成功后置位。
- **m4**［qa m-1 + pm m-2］数值口径漂移：AC10 8 线程 vs T-17 N=10；AC6 循环 100 次 vs T-09 50-100 轮；AC7 <500ms vs T-11 <60% 串行和（主判据应是锁时序记录）。
- **m5**［qa m-2］AC7 要求 list 与 create_dir 都不发生在锁内，T-11 只给 create_dir 注入延迟 → SlowMockRemote 应同时延迟两操作。
- **m6**［qa m-3］AC3 集成级正路径（/、/index.html、/assets/app.js → 200）无独立 T 行 → 并入 T-05 harness 顺带覆盖。
- **m7**［pm m-5 + qa m-8］T-04 限定"**非认证类** Err 原样透传"（call() 对 Auth 失效走切换）；AC2 的认证失效→通道切换补切换断言。
- **m8**［pm m-6 + dev m1］AC13 措辞假红：断言子串"配置解析失败"与 load_from 实际消息"配置文件解析失败 {path}"（config.rs:106）、架构 context"配置文件加载失败"均不匹配 → 统一断言子串（建议"解析失败"+路径）；"文件不可读"裁决后写入 AC13 文本。
- **m9**［dev m2］`Error::Remote` 不在 is_retryable（error.rs:69-75）→ 架构"沿既有执行器重试语义进重试队列"措辞误导：明确轮内退避用 `Error::Http{status}` 或扩展 is_retryable，或接受仅下轮重计划。
- **m10**［dev m3 + qa m-7］决策 #1"零新依赖（含 dev-deps）"与 T-03 用 cloudmount-sync::testutil::MockRemote 冲突（需 driver115 加 dev-dep，连带 rusqlite 构建）→ 改在 router.rs 内扩展现成 Stub；strategy 中"ProviderRouter 测试注入口"缺口条目过时（router.rs:54-67 `new` 本就收 `Box<dyn Provider>`），关闭。
- **m11**［dev m5］UNC 现状经 `trim_start_matches('/')`（webdav.rs:923）剥前导斜杠、并不逃逸；红测向量选盘符（`C:/x`、`C:\x`）；UNC 段拒绝保留为纵深防御。
- **m12**［dev m6］config.toml 前向兼容反向窗口：新 CLI 写入 `[cookie]` 段后，**旧版** daemon/desktop（deny_unknown_fields）读同份配置将解析失败（旧 daemon 还叠加 F-13 静默回退）→ 升级开放问题补记。
- **m13**［dev m7 + pm m-4］F-04 范围口径：cli main.rs:233 把原始 cookie 回显进错误输出（不变量 #2 已知反例，顺手脱敏或显式记 P2）；AC4 金丝雀清单（userkey/STS）窄于不变量 #2（含 Cookie 值 UID/CID/SEID/KID、token）→ 扩清单或注明本期固化 F-04 三处（已核实 cloudmount-auth/cookie.rs 无 UID/CID/SEID 值的日志点）。
- **m14**［qa m-5］OSS 注入字段语义：存完整基址（含 scheme）或改写 format!，否则本地明文 mock 打不通（与 MA3④ 同源，修订时一并落）。
- **m15**［qa m-6］T-13 spool 断言针对本次 PUT 的具体 `cm-dav-put-*` 路径，勿全目录 glob（cargo 并行测试互踩假红/假绿）。
- **m16**［qa m-9］AC5/AC13 的进程级断言（非零码退出、输出含路径）无进程级测试点 → 明确"接受单元代理"（提取 main Result 边界断言 Err 链）或补黑盒用例。
- **m17**［arch MIN-5］F-13 fail-fast 作用于全部子命令（含 service stop/uninstall，有 sc.exe 兜底）→ 边界写明或错误信息提示兜底途径。
- **m18**［arch MIN-6b］F-04 施工顺带清扫 webdav.rs:796 get_info 原始 JSON debug 日志（非凭据，同类原样打印）。

## 结论

**2 BLOCKER / 11 MAJOR / 18 MINOR。** 四角色对源码的抽验零失实，三工件方向与逐项设计可施工；BLOCKER/MAJOR 全部为测试规格、验收口径与范围裁决问题，一轮文档修订可消解。建议：**打回修订**——点名 pm（AC 回填：B1/MA9/MA10/MA11/m8/m13、开放问题补全）、architect（seam 补全：MA1/MA4/MA6、与 qa 统一 F-14 机制：MA3、桌面范围落点：MA2）、qa（向量修正：B2/MA7、映射回填：MA5、CI 语义：MA8/m5/m6/m15）；复审重点核对 B1/B2 是否消解。不修订直接放行将把"B2 空绿安全测试 + CI 门控缺口"带进 Construction。
