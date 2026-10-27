# Architecture: b2-115-qualify

> 产出者：architect　日期：2026-10-18　来源：intent.md + prd.md
>
> 定位：**人类操作为主 + 工程侧零代码**的资质申请批。本 artifact = 申请材料技术规格 + 接入预备蓝图（纯文档产出，零产品代码变更）。route=classic。
> 修订：R1（gate1 打回修订第 1 轮，2026-10-27）——数据模型节落 tracking.md 载体规格（gate1 人类裁决「tracking.md（推荐）」）/ 下游输入清单①分面表述 / 规格表 #6 #7 #10 锚与表述修订。详见 reviews/gate1-ai.md @ 7cf99dc。

## 逻辑架构

**参与方与工件流**（本批无新增运行时组件，只有协作边界）：

```text
代码事实（只读）                     工程侧产出（本批）              人类/平台
┌─────────────────────┐   提取   ┌──────────────────────┐  填表  ┌─────────────┐
│ auth/pkce.rs        │ ──────→ │ §申请材料技术规格      │ ────→ │ 115 开放平台 │
│ auth/open.rs        │         │ (architecture.md)     │  提交  │ (外部审核方) │
│ driver115/open.rs   │         ├──────────────────────┘        └──────┬──────┘
│ core/config.rs      │         │ §受理跟踪状态机 → 跟踪记录            │ AppID
└─────────────────────┘         ├──────────────────────┐               ▼
                                │ §AppID 接入预备       │ ←─── 下游 intent「AppID 实测」
                                └──────────────────────┘      (ROADMAP.md:16, 独立批)
```

- **组件**：①申请材料技术规格（本文件核心节，从代码事实逐项提取）②受理跟踪记录（载体与键值约定见数据模型节）③AppID 接入预备清单（下游 intent 的输入）。
- **边界**：工程侧不触碰平台表单、不写产品代码、不触 `.env`/密钥类；平台条款以**官方页面实测时点为准**（intent.md:18），工程侧只整理已证实事实、未知项显式标注不猜测。
- **依赖**：本批零外部工程依赖；下游「AppID 实测」依赖本批产出 AppID + 本文件 §AppID 接入预备。
- **受影响系统面（全部只读，零修改）**：`crates/cloudmount-auth/src/pkce.rs`、`crates/cloudmount-auth/src/open.rs`、`crates/cloudmount-driver115/src/open.rs`（TODO(verify) 两处所在——**勘误：编排消息写 auth/open.rs:274,371 有误，实际锚点为 driver115/open.rs:274,371，与 intent.md:13 一致，下游 intent 以此为准**）、`crates/cloudmount-core/src/config.rs`。

## 申请材料技术规格（核心产出）

从代码事实逐项提取，每项附代码锚。**标「提交时按平台页面核对」的项为未证实项，人类填表时以平台实际表单为准。**

| # | 表单项 | 工程侧口径（代码事实） | 代码锚 |
|---|---|---|---|
| 1 | 应用名称 | 建议含项目名（如 "CloudMount"）；最终名称与简介属申请主体信息——**人类裁决**（开放问题） | intent.md:23 |
| 2 | 应用类型/授权模式 | **PKCE + 设备码模式，公开客户端（无需 AppSecret）**——全流程仅 `client_id`+PKCE，无 client_secret 交换步骤 | `auth/open.rs:3`（「PKCE 设备码，无需 AppSecret」）、`open.rs:306-310,380-383` |
| 3 | code_challenge 构造 | `BASE64URL-NOPAD(SHA256(code_verifier))`；挑战固定 43 字符、无 `=` padding；RFC 7636 附录 B 官方向量测试通过 | `pkce.rs:3,62-65`（`URL_SAFE_NO_PAD` @ :64）；测试 `pkce.rs:73-80,90-94` |
| 4 | code_challenge_method | 代码字面值 **`"sha256"`**（小写）。注：RFC 7636 官方方法名为 "S256"，两者指同一算法（SHA-256 + BASE64URL-NOPAD），**语义等价已由测试向量证实**——若平台表单要求字面 "S256"，人类提交时按平台口径填写，工程侧实现无需改 | `pkce.rs:4,26,49,58`；审计小发现①（P2，`audit-findings.md` §1.0 命名微差口径） |
| 5 | verifier 长度与字符集 | 官方要求 43–128 字符；本项目固定 **64** 字符；字符集 RFC 3986 unreserved（`A-Z a-z 0-9 - . _ ~`）；OS CSPRNG 随机生成 + 拒绝采样消除模偏置（拒绝边界 198=66×3）；**明确拒绝 p115client 的 `"0"*64` 固定 verifier 偷懒默认值** | `pkce.rs:4,14-18,31-46`；`pkce.rs:7-8` |
| 6 | token 刷新机制 | ①**单飞**（single-flight）：同一把锁合并并发刷新，网络往返持锁、双检命中共享他人结果 ②**5 分钟提前量**（`REFRESH_MARGIN_SECS=300`，剩余不足即刷）③refresh_token **单次轮换**（每次刷新返回新 rt 覆盖旧值）④**40140117（刷新过频）退避不重试** ⑤**40140123–26（token 失效）强刷后重试一次** | `open.rs:39-40,54-56`（提前量）、`open.rs:178-200,218-245`（单飞）、`open.rs:23`（40140117）、`driver115/src/client.rs:216-250` + `core/src/envelope.rs:71`（失效重试，审计 L11 行证据） |
| 7 | scope | **零 OAuth scope 请求参数**（全库 grep 仅 std::thread::scope 标准库命中 5 处，非 OAuth）——认证请求只发 client_id/challenge/method。若平台表单有 scope/权限范围字段：**提交时按平台页面核对**，勾选范围以本表 #9 接口面为界 | grep 证据（std::thread::scope 5 处标准库命中）；`open.rs:306-310` |
| 8 | redirect_uri / 回调地址 | **设备码流程无回调**——代码中无 redirect_uri 参数（grep 全 crates 零命中），扫码确认后经 `deviceCodeToToken` 直接换 token。若平台表单强制要求填回调地址：**提交时按平台口径现场裁决**（开放问题） | `auth/open.rs:5-18`（流程图无回调环节）；grep 证据 |
| 9 | 所需接口面清单 | 见下方 15 端点枚举（认证域 3 + 二维码域 1 + 资源域 11）——申请时如需申报 API 权限面，以此为准 | 本节下表 |
| 10 | 请求编码 | 认证域统一 `application/x-www-form-urlencoded`；资源域列表/详情 GET、写操作 POST 实况亦为 **urlencoded**（`send_form`） | `open.rs:24,148-166`；`driver115/src/client.rs:137`（urlencoded 实况）。⚠`driver115/open.rs:5-17` 文件内表格标「multipart」与实现矛盾——收录为**未在案文档漂移候选**（供 B 系文档批修订，非本批范围） |
| 11 | 二维码有效期 | 115 未明确给出，工程按 **5 分钟（300s）**处理，失效后重新发起 | `open.rs:327-328` 注释自证 |

**接口面清单（按代码实际调用点枚举，非文档转抄）**：

| 域 | 方法 | 端点 | 用途 | 锚 |
|---|---|---|---|---|
| 认证（passportapi.115.com） | POST | `/open/authDeviceCode` | 发起设备码（client_id+challenge） | `auth/open.rs:302` |
| 认证 | POST | `/open/deviceCodeToToken` | 扫码确认后换 token（uid+verifier） | `auth/open.rs:377` |
| 认证 | POST | `/open/refreshToken` | 刷新 token（单飞+轮换） | `auth/open.rs:204` |
| 二维码（qrcodeapi.115.com） | GET | `/get/status/` | 长轮询扫码状态（1=已扫待确认/2=已确认/0=失效） | `auth/open.rs:341,356,372` |
| 资源（proapi.115.com） | GET | `/open/ufile/files` | 列表（分页 1000/页+usertime 排序）+ 变更探测 | `driver115/open.rs:219,424` |
| 资源 | GET | `/open/folder/get_info` | 目录详情 | `driver115/open.rs:251` |
| 资源 | POST | `/open/ufile/downurl` | 下载直链（pick_code） | `driver115/open.rs:264` |
| 资源 | POST | `/open/folder/add` | 新建目录 ⚠TODO(verify) | `driver115/open.rs:276` |
| 资源 | POST | `/open/ufile/update` | 重命名 ⚠TODO(verify) | `driver115/open.rs:373` |
| 资源 | POST | `/open/ufile/move` | 移动 | `driver115/open.rs:385` |
| 资源 | POST | `/open/ufile/copy` | 复制 | `driver115/open.rs:395` |
| 资源 | POST | `/open/ufile/delete` | 删除 | `driver115/open.rs:409` |
| 资源 | POST | `/open/upload/init` | 上传初始化（秒传判定） | `driver115/open.rs:324` |
| 资源 | GET | `/open/upload/get_token` | OSS 上传临时凭证 | `driver115/open.rs:147` |
| 资源 | GET | `/open/user/info` | 用户/配额（rt_space_info 解析） | `driver115/open.rs:414` |

## AppID 接入预备

下游「AppID 实测」intent（ROADMAP.md:16）的输入清单。本批只列条件，不执行。

**TODO(verify) 逐条内容与闭环条件**（实际锚点：`crates/cloudmount-driver115/src/open.rs`）：

| # | 锚点 | TODO 原文内容 | 端点与推断参数 | 闭环条件 |
|---|---|---|---|---|
| 1 | `driver115/open.rs:274` | 「参数名按官方文档推断（pid/file_name），M0 实测时确认」 | POST `/open/folder/add`，form: `pid`+`file_name` | 真实 AppID 调用建目录成功且响应可提取目录 ID（`extract_id` @ `open.rs:279-280`）；与 ROADMAP L16「确认 copy/新建目录的响应字段」直接对应 |
| 2 | `driver115/open.rs:371` | 「参数名（file_id/file_name）待 M0 实测确认」 | POST `/open/ufile/update`，form: `file_id`+`file_name` | 真实 AppID 改名成功且无参数类错误码 |

**AppID 注入点（config.rs 事实）**：

- **配置键**：顶层 `app_id`（`Option<String>`，默认 `None`）——`config.rs:31`（注释自证「申请 https://open.115.com/」）、`:102`。
- **配置文件位置**：数据目录下 `config.toml`——Windows `%APPDATA%\cloudmount\config.toml`／macOS `~/Library/Application Support/cloudmount/config.toml`／Linux `$XDG_DATA_HOME/cloudmount/config.toml`（默认 `~/.local/share/...`）；可用 `CLOUDMOUNT_DATA_DIR` 重定向——`config.rs:4-12,116-118`。
- **env 覆盖**：`CLOUDMOUNT_APP_ID`（env > 文件；**空值不覆盖**文件值）——`config.rs:157-160`；全量优先级 `CLI flag > env > 配置文件 > 内置默认`——`config.rs:16-17`。
- **防错**：`deny_unknown_fields`——键名拼错显式报错不静默——`config.rs:28`。
- **消费点**：`OpenApiAuth::new(app_id, store)`——`auth/open.rs:89`。
- **CLI 消费面差异（gate1 M2 修订）**：CLI `login` 的 AppID 仅来自 clap flag `--app-id` 或 env `CLOUDMOUNT_APP_ID`（`cli/main.rs:33-34`），**不读 config.toml**（:143-150 两者皆缺即报错）；`config.toml` 的 `app_id` 键路线仅对 daemon/desktop 生效。

**下游输入清单**（AppID 到位后依次执行）：①人类从平台获取 AppID → **分面注入**：CLI login 须 `--app-id` flag 或 `CLOUDMOUNT_APP_ID` env（cli main.rs:33-34/:143-150 不读 config.toml）；config.toml `app_id` 键路线仅 daemon/desktop 生效 → ②实测上表两处 TODO(verify)，确认参数名与响应字段 → ③按实测结果修订 `driver115/open.rs:274,371` 注记 → ④勾选 ROADMAP.md:16。**全部属下游 intent，本批零执行。**

## 受理跟踪状态机

供 pm 采纳入 AC 的跟踪口径建议。

```text
[draft 未提交] → [submitted 已提交] → [accepted 已受理·审核中] ─┬→ [approved 通过·得 AppID]
                     │ 记录：提交日期+凭据        │ 记录：受理号      └→ [rejected 驳回] →（补材料）→ submitted
                     └────────── 超期分支：escalated（escalate_date 到期未果，人类裁决去向）
```

- **跟踪记录字段约定**（载体与完整键值见数据模型节）：`submit_date`（提交日期，ISO 8601）、`receipt_no`（平台受理号/工单号）、`status`（六态，draft 态不建记录）、`expect_date` = submit_date + 7 工作日（首次审核）、`escalate_date` = submit_date + 14 工作日（7+7 口径，源自 ROADMAP.md:3-4「审核约 7+7 工作日」）、`check_method`（受理状态查看方式：页面入口+查看凭据，承载 prd AC3②）。
- **记录位置**：`docs/aidlc/b2-115-qualify/tracking.md`（gate1 裁决钉定；数据模型节为建文件规格）——AIDLC 工件可追溯、仓库内、无敏感明文。
- **超期升级建议**：`escalate_date` 到期仍未果 → 触发人类裁决（催办/工单/换道）——工程侧只给触发器与时点计算，**去向属开放问题**（intent.md:24）。工程侧零自动动作。

## 数据模型

无数据库/持久化实体（零代码批）。**受理跟踪载体（gate1 人类裁决）：`docs/aidlc/b2-115-qualify/tracking.md`**——独立 Markdown 文档、追加式日志（多次更新不改写旧行，保留审计历史）。建文件规格：

| 键 | 类型 | 约束 |
|---|---|---|
| `submit_date` | 日期（ISO 8601） | 提交当日记录；不可空 |
| `receipt_no` | 字符串 | 平台受理号/工单号原样记录；不可空（若无受理号机制则记提交凭据截图存放位置说明） |
| `status` | 枚举 submitted/accepted/approved/rejected/escalated | **六态映射：`draft`=尚不建记录，首条记录自 submitted 起**（draft 态无提交日期，与 submit_date 不可空不冲突 [dev-m6]）；状态迁移按状态机节 |
| `expect_date` / `escalate_date` | 日期 | 由 submit_date 推导（+7/+14 工作日），非平台承诺 |
| `check_method` | 自由文本 | 受理状态**查看方式**（gate1 裁决新增键，承载 prd AC3②）：平台页面入口（在哪个页面看）+ 查看凭据（以什么账号/入口查看）；不可空 |
| `notes` | 自由文本 | **禁止记录**密钥/凭证明文/实名证件信息（安全节约束） |

- **多次更新语义**：status 每次变更**追加新行**（新行含变更后 status 与日期），不改写旧行——追加式日志保留完整审计历史。
- AppID 本身在下游才落 `config.toml`（AppID 接入预备节）；跟踪记录中可记 AppID 值（非密钥），但不与任何 secret 同处记录。

## 与 PRD 的语义对齐表

| PRD 动词 | 物理语义 | 说明 |
|---|---|---|
| 整理（申请材料技术清单） | 产出本文件 §申请材料技术规格——逐项代码锚，零代码变更 | 提取源仅四处代码事实（pkce.rs / auth/open.rs / driver115/open.rs / config.rs） |
| 提交（资质申请） | 人类在 115 开放平台表单执行；工程侧零动作，仅在 tracking.md 追加 `submit_date`+`receipt_no` 行 | 敏感边界：材料不含密钥/凭证明文 |
| 跟踪（受理状态） | 按状态机节在 tracking.md 追加新行更新 `status` 与日期字段；`check_method` 记录查看方式；不改产品代码 | 7+7 工作日时点自动可推算，升级动作归人类 |
| 预备（AppID 接入） | 产出 §AppID 接入预备 清单（两处 TODO(verify) 闭环条件 + config 注入点 + 分面注入口径）；AppID 到位前不执行任何实测 | 本批只预备，执行归下游 intent |
| 闭环（TODO(verify)） | 下游 intent 按 §AppID 接入预备 的闭环条件实测并修订代码注记 | 本批范围外，显式划出 |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| 1 | 零产品代码变更，artifact 纯文档 | intent.md:17 硬约束「人类操作为主，工程侧零代码」（审核报告 §3.1 原判沿袭）；「AppID 实测」为独立下游 intent（intent.md:20） |
| 2 | method 值按代码事实写 `"sha256"` 并标注与 "S256" 的字面差异 | 审计小发现①已定性为「协议措辞 vs 代码命名」（P2 非证伪）；RFC 7636 中 S256 即本实现的 SHA-256+BASE64URL-NOPAD，测试向量（pkce.rs:73-80）证语义等价；平台表单字面要求不可猜（intent.md:18），留人类提交时核对 |
| 3 | 接口面清单从代码调用点枚举，不转抄官方文档 | 只有实际调用的 15 端点才是申报权限面的真实依据；官方要求可能更新（intent.md:18），文档转抄会引入未证实条款（driver115/open.rs:5-17 文件内表格 multipart 漂移即转抄风险实证，规格 #10） |
| 4 | scope/redirect_uri/应用名称等未知项显式标「提交时按平台页面核对」 | grep 证实工程侧零 OAuth 参数使用（规格 #7/#8）；不猜测未证实条款是 intent 硬约束（intent.md:18） |
| 5 | 受理跟踪载体钉定 tracking.md，追加式日志 | gate1 人类裁决「tracking.md（推荐）」（reviews/gate1-ai.md M1）；独立文档可变键值、可追溯；status 多次更新追加新行不改审计历史 |
| 6 | PKCE 应用按「公开客户端」口径申报 | 代码事实：authDeviceCode 仅 client_id+challenge（open.rs:306-310）、deviceCodeToToken 仅 uid+verifier（open.rs:380-383）、模块注释自证「无需 AppSecret」（open.rs:3）——无客户端密钥分发面 |
| 7 | 勘误并锁定 TODO(verify) 锚点为 driver115/open.rs:274,371 | 编排消息误写 auth/open.rs；intent.md:13 与实际代码（grep 证实）一致——下游 intent 用错锚点会闭环失败，须在本蓝图一次纠正 |
| 8 | 下游输入清单按 CLI/daemon-desktop 分面表述 AppID 注入 | gate1 M2：CLI 二进制不读 config.toml 的 app_id（cli/main.rs:33-34/:143-150）——单一表述会令下游 intent 第一步即失败；分面表述零代码修 |

## 安全与权限

- **申请材料零密钥**：材料清单不含任何密钥/凭证明文（intent.md:19 约束；`.env`/密钥类零触碰，guard-paths 照常）。
- **公开客户端模式**：PKCE 流程无 client_secret（决策 #6 代码佐证）；verifier 仅存内存（`PendingLogin` @ auth/open.rs:60-69，非 Serialize、不落盘），换取 token 后即弃。
- **凭据持久化面（佐证，本批无接触）**：token 经 `SecretStore` 落 Windows 凭据管理器/macOS Keychain/Linux 0600 文件，keyring 服务名 `io.github.tintac-cn.cloudmount`（store.rs:145，审计 L10 行证据）。
- **最小权限三元组**（本批无 IAM，以授权边界表述）：
  - （人类·项目所有者）+（115 开放平台账号与应用）+（提交申请/创建应用/响应审核）——**唯一**对外操作执行者；
  - （AIDLC 工程角色）+（`docs/aidlc/b2-115-qualify/` 工件区）+（写本文件及 tracking.md）——代码面全只读，零产品代码写；
  - （未来 PKCE 应用）+（115 OpenAPI 资源域）+（上表 15 端点最小接口面）——scope 未使用，权限面以实际调用为界，申请时不多勾。

## 开放问题（需人类裁决）

- [ ] 申请主体信息：个人开发者实名信息与应用名称/简介文案——人类线下持有与定稿，工程侧仅留清单占位（intent.md:23 沿袭）。
- [ ] 受理超期升级口径：`escalate_date`（提交+14 工作日）到期未果 → 平台催办/工单/换道？——人类裁决（intent.md:24 沿袭）。
- [ ] code_challenge_method 表单字面口径：平台若要求字面 "S256"（实现为 "sha256"，语义等价已证）——提交时按平台页面核对，人类现场定填写值。
- [ ] redirect_uri 是否平台必填：设备码流程无回调（代码事实）；若表单强制，填写口径人类按平台页面现场裁决。
- [ ] scope/权限范围勾选口径：工程侧零 scope 使用；若平台要求勾选，按 §申请材料技术规格 #9 接口面清单对勾，最终以平台页面实际选项为准。
