# AppID 接入预备清单（b2-115-qualify U3 产出）

> 产出者：dev（Construction U3）　日期：2026-10-28　基线：6f9733e（U2-done）
> 来源：`architecture.md`「AppID 接入预备」节（R1 修订版，gate1 M2 分面口径）+「下游输入清单」①——蓝本已有的锚点抽验复用、全部落笔前实读复核。
> 下游消费者：**「AppID 实测」intent**（`docs/ROADMAP.md:16` 闭环——「待 AppID 实测：真实账号跑通全链路；确认 copy/新建目录的响应字段」；独立下游批，`intent.md:20`）。本批只列条件，**零执行**（intent.md:17 零代码）。
> 敏感边界：本清单零密钥/凭证明文（intent.md:19）；AppID 值属下游到位后才记录的**非密钥**值，本清单只出现占位符形态。

## 一、两处 TODO(verify) 逐条（闭环条件+相邻对照面）

### 1.1 动态清点（TP-04 口径）

`grep -rn "TODO(verify)" crates/` 实测（2026-10-28 @ 6f9733e）：**恰 2 处**，均在 `crates/cloudmount-driver115/src/open.rs`（:274 / :371）。auth 侧 `open.rs` 无 TODO(verify)（test-strategy.md:8 口径——两类锚勿混淆；architecture.md 决策 #7 勘误锁定锚点为 driver115，编排消息曾误写 auth/open.rs）。动态清点不锚全仓硬编码——`docs/aidlc/**` 工件留痕含该字面且随 gate 增长。

### 1.2 逐条内容与闭环条件

| # | 锚点 | TODO 原文（逐字） | 端点与推断参数 | 闭环条件（实测后改代码+删注） |
|---|---|---|---|---|
| 1 | `driver115/open.rs:274` | 「参数名按官方文档推断（pid/file_name），M0 实测时确认。」 | POST `/open/folder/add`，form：`pid`+`file_name`（:275-277） | ①真实 AppID 调用建目录成功；②响应可提取目录 ID——`extract_id` 调用点 :279-280（宽松提取键位 `file_id`/`fileId`/`cid`/`fid`/`fid_list`，fn 定义 :102-104），取不到即报错「新建目录响应未返回 ID」；③按实测结果修订注记（参数名与推断不符即改 form 字段）并**删除 TODO 注释**。与 ROADMAP.md:16「确认 copy/新建目录的响应字段」直接对应 |
| 2 | `driver115/open.rs:371` | 「参数名（file_id/file_name）待 M0 实测确认。」 | POST `/open/ufile/update`，form：`file_id`+`file_name`（:372-377） | ①真实 AppID 改名成功且**无参数类错误码**；②按实测结果修订注记（参数名与推断不符即改 form 字段）并**删除 TODO 注释** |

### 1.3 相邻对照面（已确认 vs 待实测——下游 intent 的验证清单）

| 操作 | 锚点 | 参数状态 | 实况 |
|---|---|---|---|
| move | `driver115/open.rs:382-389` | **已确认**（:383 注释自证） | POST `/open/ufile/move`：`file_ids`（逗号分隔）+ `to_cid`（目标目录，根为 0） |
| copy | `driver115/open.rs:391-403` | 参数**已确认**（:392-393 注释自证）；**响应字段待实测**（:398-402） | POST `/open/ufile/copy`：**单数** `file_id` + 目标目录 `pid`（与 move 的 file_ids/to_cid 不同）；`nodupli` 可选（1=不允许重复）。响应只知 `data: array`，当前宽松提取（复用 `extract_id`），取不到报错「复制成功但响应未返回新文件 ID（待实测确认）」——**ROADMAP.md:16 待实测项** |
| delete | `driver115/open.rs:405-411` | **已确认**（:406 注释自证） | POST `/open/ufile/delete`：`file_ids` 为**单个字段**、多个 ID 用英文逗号分隔；响应丢弃不解析（prd AC1⑧ 口径：非 ROADMAP.md:16 待实测项） |

> 对照面语义：move/copy/delete 三操作参数名已在案（注释自证），下游实测只需聚焦 §1.2 两处 TODO 参数名 + copy 响应 ID 字段——**清单不虚增待实测项**（TP-04 证伪判据）。

## 二、config 注入点分面（gate1 M2 裁决口径）

> 裁决源：gate1-ai.md M2（architecture.md 决策 #8）——CLI 二进制不读 config.toml 的 `app_id`，单一表述会令下游 intent 第一步即失败；分面表述零代码修。

### 2.1 CLI login 分面：`--app-id` flag 或 `CLOUDMOUNT_APP_ID` env（**不读 config.toml**）

- AppID 仅两来源：clap flag `--app-id`（global）或 env `CLOUDMOUNT_APP_ID`——`cli/main.rs:33-34`（`#[arg(long, global = true, env = "CLOUDMOUNT_APP_ID")] app_id: Option<String>`）。
- 两者皆缺即报错——`cli/main.rs:143-150`（`build()`：「缺少 AppID：请设置环境变量 CLOUDMOUNT_APP_ID 或传 --app-id」+ 申请入口提示 https://open.115.com/ ）。
- **不读 config.toml 的实证**：CLI 全文件唯一 `config::load` 调用在 `persist_cookie_consent`（`cli/main.rs:308-309`）——仅 Cookie 同意写回（`[cookie]` 双真，:311-312），不触碰 `app_id` 键 → config.toml 的 `app_id` 对 CLI 无效。
- 用法示例（模块注释自证，`cli/main.rs:7`）：`export CLOUDMOUNT_APP_ID=<开放平台 AppID>` 或 `--app-id`。
- 相邻注记（非第三注入点）：`status`/`logout` 走 `build_auth`（`cli/main.rs:157-161`）`unwrap_or_default`——AppID 空串即可（只读 token、不发认证请求）。

### 2.2 config.toml `app_id` 键路线：**仅 daemon/desktop 生效**

- **daemon**：`apply_config_defaults` 把文件值注入 clap default——`daemon/main.rs:266-267`（`if let Some(v) = cfg.app_id.clone() { cmd = cmd.mut_arg("app_id", ...) }`）；优先级注释自证「CLI > env > 文件 > 内置」（:258）。消费点：`:333`（转发 `--app-id` 给子进程）/ `:512-515`（缺失报错「缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id」）/ `:438-440`（`build_open_provider` → `OpenApiAuth::new(app_id, store)`）。
- **desktop**：引擎线程 `load_effective().unwrap_or_default()` → `cfg.app_id.clone()`（`desktop/main.rs:784-785`）→ `OpenApiAuth::new(id, store)`（`:798-800`，`app_id.and_then` 内构造）。

### 2.3 优先级链 / 三平台路径表 / env 覆盖（空值不覆盖守卫）

- **配置键**：顶层 `app_id: Option<String>`，默认 `None`——`config.rs:30-31`（doc 注释指向申请入口 https://open.115.com/ ）、`:102`（Default 实现）。
- **优先级链**：`CLI flag > 环境变量 > 配置文件 > 内置默认`——`config.rs:16-17`（模块注释自证）。
- **三平台配置路径表**（`config.rs:6-12`；路径解析 `config_path()` :116-118——数据目录下 `config.toml`）：

| 平台 | 路径 |
|---|---|
| Windows | `%APPDATA%\cloudmount\config.toml` |
| macOS | `~/Library/Application Support/cloudmount/config.toml` |
| Linux | `$XDG_DATA_HOME/cloudmount/config.toml`（默认 `~/.local/share/...`） |

  可用 `CLOUDMOUNT_DATA_DIR` 重定向（容器/边缘部署）——`config.rs:12`。
- **env 覆盖**：`CLOUDMOUNT_APP_ID`（env > 文件）——`config.rs:157-160`；**空值不覆盖**守卫 `if !v.is_empty()` @ `:158`（CI 常见空变量不吞文件值，测试 `empty_env_keeps_file_value` 在案）。`load_effective` = load + env 覆盖——`config.rs:176-178`。
- **防错**：`deny_unknown_fields`（`config.rs:28`）——键名拼错显式报错不静默。

## 三、认证侧注入

- **构造**：`OpenApiAuth::new(app_id, store)`——`auth/open.rs:89`（`pub fn new(app_id: impl Into<String>, store: Arc<dyn SecretStore>)`）；token 从 `store` 恢复（:102-110，`SecretKind::OpenApiToken`）。
- **client_id 注入**：发起设备码时 AppID 作为 `client_id` 表单字段——`auth/open.rs:307`（form 三元组 `client_id`+`code_challenge`+`code_challenge_method`，:304-312；POST `/open/authDeviceCode` @ :302）。
- **三二进制汇聚点**：CLI `cli/main.rs:151` / daemon `daemon/main.rs:440` / desktop `desktop/main.rs:800` 全部经 `OpenApiAuth::new` → `client_id`——AppID 的唯一认证侧注入口，无其它注入点（TP-04「不虚增」口径）。

## 四、下游「AppID 实测」intent 输入清单（本清单+tracking.md AppID 值）

AppID 到位后依次执行（**全部属下游 intent，本批零执行**——intent.md:20）：

1. **人类从平台获取 AppID** → 记录 AppID 值于 `docs/aidlc/b2-115-qualify/tracking.md` `notes` 键（AppID 非密钥可记、**不与任何 secret 同处**——tracking.md:7 / architecture.md 数据模型节；追加新行不改写旧行）。
2. **分面注入**（本清单 §二）：
   - CLI login：`--app-id` flag 或 `CLOUDMOUNT_APP_ID` env（**不读 config.toml**——§2.1）；
   - daemon/desktop：config.toml `app_id` 键（三平台路径见 §2.3；env 空值不覆盖守卫同节）。
3. **实测 §一 两处 TODO(verify)**：确认 create_dir 参数名（推断值 `pid`/`file_name`）与 rename 参数名（推断值 `file_id`/`file_name`）+ copy 响应 ID 字段（:398-402 宽松提取待补精确字段）。
4. **按实测结果修订代码**：`driver115/open.rs:274,371` 注记（参数名与推断不符即改 form 字段）+ 删除 TODO 注释；copy 响应解析按实测字段补精确提取。
5. **勾选 ROADMAP.md:16**（`- [ ] 待 AppID 实测：真实账号跑通全链路；确认 copy/新建目录的响应字段` → `[x]`）。

> 下游 intent 的 intent.md 素材 = 本清单（§一实测项+§二注入分面+§三认证侧）+ tracking.md（AppID 值）。

## 自验记录（TP-04 / TP-05）

- **TP-04（AC4）**：`grep -rn "TODO(verify)" crates/` 动态清点**恰 2 处**（`driver115/open.rs:274`/`:371`），与本清单 §一 逐条一致（TODO 原文逐字、参数名、闭环条件、对照面锚点均实读核对）；注入点（config 分面+认证侧）逐条在列且与实文一致，无虚增。**AC4①-⑤ 落字对照**：①=§1.2 #1；②=§1.2 #2；③=§1.3 对照面（move :383 已确认示例+copy/delete 超集收录）；④=§2.3+§三（config.rs:30-31/:157-160/:16-17+auth/open.rs:89/:307）；⑤=§四（分面表述+「本 intent 不执行，由下游 intent 承接（ROADMAP.md:16，含 copy/新建目录响应字段确认）」）。
- **TP-05（AC1–AC4 约束）**：`git diff --name-only 14a7eb4..HEAD` @ 6f9733e → 15 文件全在 `docs/aidlc/b2-115-qualify/**`，产品目录（crates/web-ui/apps/drivers）**零改动**；本 unit 仅新增本文件（`docs/aidlc/b2-115-qualify/appid-readiness.md`），提交后复核 diff 面仍限本目录。
- **代码锚实测计数**：落笔前逐锚实读 6 个源文件（driver115/open.rs、cli/main.rs、daemon/main.rs、desktop/main.rs、core/config.rs、auth/open.rs）；本清单字面引用的代码锚按 file:line 去重计 **44 处**（driver115 13 / cli 7 / daemon 6 / desktop 3 / config 10 / auth 5——含 :158/:307/:406 等区间内单行精锚），全部与实文一致；蓝本（architecture.md）已有锚点复用抽验、无凭蓝本转抄未核项。
