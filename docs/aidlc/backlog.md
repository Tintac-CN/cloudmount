# CloudMount Backlog（统一台账）

> 建立：2026-10-16（audit-roadmap-next-steps ops，人类裁决「归集抽入 backlog.md 建统一台账」）。
> 来源：`docs/aidlc/audit-roadmap-next-steps/audit-findings.md` §3（gate2 已审归集，行锚经准出回填实测订正）。
> 用法：`/aidlc:inception` 开新 intent 前先查此表；A2 搁置项追加于本表；完成标 `✅ 已消解（<intent-slug>）`。**本表为唯一活跃台账**（审核报告 §3 为时点快照，不再更新——裁决①）。

## A. 路线图开放项（12 条，来源=报告 §3.1）

| 日期 | 开放项 | 来源锚 | 阻塞 | 最小 intent 粒度 | 状态 |
|---|---|---|---|---|---|
| 2026-10-16 | 申请 115 开放平台个人开发者+创建 PKCE 应用（**外部阻塞链源头**，审核约 7+7 工作日） | ROADMAP.md:8 | 外部 | 材料提交+受理跟进（人类操作为主） | 待启动（**B2 批次**） |
| 2026-10-16 | 待 AppID 实测：真实账号全链路跑通+copy/新建目录响应字段确认（open.rs:274,371 TODO(verify) 闭环） | ROADMAP.md:16 | 外部（间接，依赖上项） | 真实凭据全链路实测 intent（可与真实凭据联调合并） | 待前置 |
| 2026-10-16 | 待真实凭据联调：两设备干净双向同步 | ROADMAP.md:35 | 外部（间接，同依赖） | 并入上项 intent | 待前置 |
| 2026-10-16 | 验收指标：空闲 RSS ≤ 20MB 观测（cloudmountd --once） | ROADMAP.md:36 | 内部 | 小 intent：观测脚本+留痕（可与 AppID 实测合并） | 待排 |
| 2026-10-16 | MSIX 打包（可选，仅 Store 渠道） | ROADMAP.md:133 | 内部（Store 决策） | 独立 intent（仅当 Store 分发被裁决） | 长期搁置 |
| 2026-10-16 | Cookie 独有能力：分享/转存/批量（二级入口+常驻风险提示） | ROADMAP.md:154 | 内部 | 单独 intent（**B3 批次**） | 待排 |
| 2026-10-16 | macOS File Provider 扩展（Swift 薄桥+core staticlib；**先裁决 core ABI 冻结**） | ROADMAP.md:181 | 外部（账号/真机）+内部（ABI 裁决） | PoC intent→扩展 intent 两段式（**B7 批次**） | 待裁决 |
| 2026-10-16 | 自研 Windows minifilter（长期；设计定稿 18–30 人月） | ROADMAP.md:186-194 | 内部（长周期，不阻塞产品） | 按 KM-0 起分段 intent | 长期轨道 |
| 2026-10-16 | 【本期新增】M4 验收实测：512MB 设备 48h 稳定+局域网 WebDAV 真机读写 | 报告 §1.6/§4.2 | 内部 | 小 intent：soak 观测+留痕（**B6 批次**） | 待排 |
| 2026-10-16 | 【本期新增】文档纠偏批：stale Tauri 全量面（16 处，含 README:151 死命令）+owner 元数据+.gitignore 死路径+TLS 注释 | 报告 §2.1①④+§2.2 NEW-1..3/NEW-5 | 无（零依赖） | 单独 intent：纯文档/元数据回写（**B1 批次——已裁决下一步启动**） | **🚀 已裁决启动** |
| 2026-10-16 | 【本期新增】部署面主从收敛裁决（edge/ vs packaging/ 已分叉） | 报告 §2.2 NEW-4 | 内部（裁决后开工） | 裁决→小 intent（声明废弃方或合并） | 待裁决 |
| 2026-10-16 | 【本期新增】Intel mac 无 desktop 交付补登（x86_64 dmg 是否需要） | 报告 §4.2#3 | 内部 | 需要：dmg job 加 target；不需要：文档声明 | 待裁决 |

## B. 四 intent backlog（37 条，来源=报告 §3.2；行锚已实测订正，详见报告）

### 工程/测试/发布（可开 intent 的实质项）

| 日期 | # | 条目 | 来源 | 分类 | 状态 |
|---|---|---|---|---|---|
| 2026-10-16 | 1 | ci.yml 增 actionlint 步骤（三源重复已并） | macos-release/installer-packages/desktop-installers runbook | CI | 待排 |
| 2026-10-16 | 2 | Cargo.toml repository 元数据修正（zhonghaoxi→tintac-cn；=NEW-5 错侧） | installer-packages/desktop-installers runbook | 文档 | **并入 B1** |
| 2026-10-16 | 3 | check-macos.yml `permissions:` 显式化（release.yml 六 build job 同类缺口可一并） | macos-release runbook | CI | 待排 |
| 2026-10-16 | 4 | 断言脚本 YAML 解析精度（job_block 启发式→真解析） | macos-release runbook | CI | 待排 |
| 2026-10-16 | 5 | package-macos.sh tar 半成品清理 | macos-release runbook | 发布 | 待排 |
| 2026-10-16 | 6 | `.gitattributes` 行尾统一（release.yml CRLF/check-macos.yml LF） | macos-release runbook | CI | retro 优先项 |
| 2026-10-16 | 12 | wxs Package Description/Comments 三件套描述（ARP 元数据欠准；需新授权） | desktop-installers runbook 备案① | 发布 | 待排 |
| 2026-10-16 | 14 | dmg 脚本不校验二进制架构（file/lipo 兜底） | desktop-installers runbook 备案③ | 发布 | 待排 |
| 2026-10-16 | 15 | TP-23 无显式承接载体（补埋点） | desktop-installers runbook 备案④ | 测试 | 待排 |
| 2026-10-16 | 16 | T4 修订断言①防空真依赖步骤名字面 | desktop-installers runbook 备案⑤ | CI | 待排 |
| 2026-10-16 | 22 | .gitignore L13-14 src-tauri 死路径清理（=D2） | desktop-installers runbook 备案⑪ | 文档 | **并入 B1** |
| 2026-10-16 | 23 | CI 补 desktop fmt/clippy 步骤（终验口径缺口上游补死） | desktop-installers retro | CI | **✅ 已消解（ci-desktop-fmt-clippy）**（INTENT-DONE 2026-10-29：ci.yml desktop job + 断言 47 条 + 六脚本硬门口径入 ops-runbook） |
| 2026-10-16 | 24 | hash.rs:157 per-run 唯一 temp 目录（间歇 flaky 头号嫌疑根治） | audit-p0p1-fixes runbook+retro | 代码 | 待排 |
| 2026-10-16 | 25 | exec.rs 错误路径 dir_creating 条目残留 | audit-p0p1-fixes runbook | 代码 | 待排 |
| 2026-10-16 | 26 | webdav.rs:513 冗余条件 | audit-p0p1-fixes runbook | 代码 | 待排 |
| 2026-10-16 | 27 | cookie_mock/mock_api ~60 行平行实现重构 | audit-p0p1-fixes runbook | 代码（重构） | 待排 |
| 2026-10-16 | 28 | cookie.rs:584 initupload 解密明文日志收口（F-04 脱敏不变量延伸） | audit-p0p1-fixes runbook+retro | 代码（安全卫生） | 待排 |
| 2026-10-16 | 35 | 伪隐藏 mac 形态二选一（Dock 留存 vs LSUIElement）随真机实测裁决 | desktop-installers retro+runbook | 代码 | 并入真机核验批（条目 31） |
| 2026-10-16 | 36 | gen_icons 扩尺寸集沿 stdlib 方案扩（条件性） | desktop-installers retro | 代码 | 条件待排 |
| 2026-10-29 | 40 | ci-desktop-yaml.test.sh 断言硬化：负断言语法变体（#12 `- if:` 列表项/#46 flow-map `env: {RUSTFLAGS: ...}`/#45 `./apps/desktop` 前缀/#10-#14 同类变体）+ `job_block` 2 空格注释截断 + #43/#44 自检端到端深度 + 头注「M 基线=47 个 report 调用点」措辞与 python/pyyaml 依赖注明（当前无违规，硬化批随下次解锁执行） | ci-desktop-fmt-clippy gate2-ai MINOR 3/4/5/10 | CI | 待排 |
| 2026-10-29 | 41 | ci-desktop job 首跑观察与门控收口：TP-06 首跑（desktop job 绿 + runner `rustc -V` 与本机对比 + cache 体积/命中率与 10GB 配额）+ branch protection 加入 desktop required checks | ci-desktop-fmt-clippy gate2-ai MINOR 12 + architecture M6 风险节 | CI | **首跑已观察**（2026-10-29 PR#12/main run 40900994862：desktop **success 3m11s**，rustc 1.99.1，cache `No cache found` 冷编 clippy 2m05s）——待 warm 跑复观测（命中率/体积）+ required checks 决策 |
| 2026-10-29 | 42 | 【首跑实测·预存在】cross aarch64-unknown-linux-musl 腿失败：cross-rs 0.2.5 镜像内 build-script 需 GLIBC 2.28/2.29/2.30（libc 0.2.190）而镜像 glibc 过旧（gettid relocation error）；合并前 origin/main 同腿已失败（run 40665993886），非本次合入引入；修复方向=升级 cross 镜像/容器或钉 rustc 版本（本批不改） | main 首跑 run 40900994862 + 合并前 run 40665993886 | CI | 待排 |

### 观察项（跟踪类）

| 日期 | # | 条目 | 来源 | 状态 |
|---|---|---|---|---|
| 2026-10-16 | 29 | macos-15 runner 退役公告跟踪（退役时三源同步改 runs-on+断言） | 三 runbook〔D:3源〕 | 观察 |
| 2026-10-16 | 30 | WiX 3.14.1.20250415 废弃公告跟踪（升 v4/v5 需重写 .wxs） | 两 runbook〔D:2源〕 | 观察 |
| 2026-10-16 | 31 | 安装包/dmg/托盘真机核验触发（首个含安装包 tag 后人工清单+伪隐藏 mac 形态裁决） | 三源〔D:3源〕 | 观察（v0.2.1 已发布——条件已触发，待排） |
| 2026-10-16 | 32 | 间歇 flaky CI 现身时按失败测试名定位（条目 24 伴生） | audit-p0p1-fixes runbook | 观察 |
| 2026-10-16 | 38 | 【B1 gate2 报告】engine_works_with_native_watcher 满载并行下原生 watcher 时序 flake 1 例（隔离复跑 5/5；277/0 基线不受影响）——重试加固或隔离标注 | b1-doc-corrections gate2-ai MINOR | 观察 |
| 2026-10-16 | 39 | 【B1 gate2 r2】prd AC1 #11/蓝图 #15 失实措辞（「8 命令已全部实现」）保留为历史工件面——补就地裁决注记或维持豁免（先例=desktop-installers 条目 21） | b1-doc-corrections gate2-ai-r2 MINOR-1 | 记录 |

### 记录性（不入工程队列）

| 日期 | # | 条目 | 来源 | 状态 |
|---|---|---|---|---|
| 2026-10-16 | 7 | state.json U2 计数口径勘误注记（时点值口径） | 两源〔D:2源〕 | 记录 |
| 2026-10-16 | 8 | decisions「移除 ICE80」措辞勘误注记 | installer-packages runbook | 记录 |
| 2026-10-16 | 9 | T15 注入自校验正分支补跑留痕（actionlint 可用环境） | installer-packages runbook | 记录（与条目 1 联动） |
| 2026-10-16 | 10 | .wxs 无 UIRef 知会（双击安装走缺省 basic UI） | installer-packages runbook | 记录 |
| 2026-10-16 | 11 | release 下载步注释语义漂移修正 | installer-packages runbook | 记录 |
| 2026-10-16 | 13 | HKCU KeyPath 多用户卸载孤儿（ICE43 经典代价，决策已留痕） | desktop-installers runbook 备案② | 记录 |
| 2026-10-16 | 17 | desktop 断言计数环境相关口径（112/113）注记 | desktop-installers runbook 备案⑥ | 记录 |
| 2026-10-16 | 18 | gen_icons 幂等跨 zlib 版本边界注记 | desktop-installers runbook 备案⑦ | 记录 |
| 2026-10-16 | 19 | TP-12 'winres' 子串宽松（现态无害） | desktop-installers runbook 备案⑧ | 记录 |
| 2026-10-16 | 20 | FFI GetWindowLongPtrW 32 位约束注释留痕 | desktop-installers runbook 备案⑨ | 记录 |
| 2026-10-16 | 21 | units/2.md 契约字面补伪隐藏偏差指针（已完结工件——经新 intent 或豁免） | desktop-installers runbook 备案⑩ | 记录 |
| 2026-10-16 | 33 | 环境备注：`cargo test -p cloudmount-sync` 无 features 编译失败为既有状态 | audit-p0p1-fixes runbook | 记录 |
| 2026-10-16 | 34 | retro 候选：release workflow 三平台自动化——**✅ 已消解**（release.yml 九 job 现状） | audit-p0p1-fixes retro | ✅ 已消解 |
| 2026-10-16 | 37 | retro 候选：AskUserQuestion 超时降级对话裁决（编排流程习惯） | 两 retro〔D:2源〕 | 记录（流程项） |
