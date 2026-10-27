# 115 开放平台申请材料技术清单（b2-115-qualify U1 产出）

> 元信息：来源=`docs/aidlc/b2-115-qualify/architecture.md`「申请材料技术规格」11 项规格表+「受理跟踪状态机」/「数据模型」节（R1 修订版，gate1 approved）；日期=2026-10-28；执行者=dev（Construction U1）。
> 产出性质：纯文档（零代码变更——intent.md:17）；本批工程侧新建文件仅本清单与 `tracking.md`。
> 代码锚纪律：全部代码锚落笔前实读复验（源文件全读 4+辅助抽读 5 文件+grep 证据 3 组——见文末「附：锚与 grep 证据实测记录」）。
> 敏感边界（TP-02）：本清单零密钥/零凭证明文/零实名证件信息（intent.md:19）——实名信息只留占位「人类线下持有」（开放问题①，intent.md:23）。
> 未知项口径（intent.md:18）：标「**提交时按平台页面核对**」的项=未证实项——平台要求以官方页面实测时点为准，工程侧不猜测未证实条款。
> 「表单字段」列说明：工程侧拟名（中文+英文占位）；115 平台表单实际字段名/选项以提交时页面为准。

## 一、表单技术参数（11 项）

| # | 表单字段（中/英占位） | 技术值（代码事实） | 代码锚（文件:行，均实测） | 未知项标注 |
|---|---|---|---|---|
| 1 | 应用名称 / App Name | 建议含项目名（如 "CloudMount"）；最终名称与简介属申请主体信息——人类裁决 | `intent.md:23`（申请主体信息开放问题）；`prd.md:52`（建议占位 "CloudMount"） | 最终名称/描述文案/图标=人类偏好裁决，工程侧不代拟终稿（现场裁决④） |
| 2 | 应用类型/授权模式 / App Type · Authorization Grant | **PKCE+设备码模式，公开客户端（无需 AppSecret）**。登录四步：`authDeviceCode`→115 App 扫码→长轮询→`deviceCodeToToken`（token 刷新为流程第 5 步）；全流程仅 `client_id`+PKCE，无 client_secret 交换步骤 | `crates/cloudmount-auth/src/open.rs:3`（「PKCE 设备码，无需 AppSecret」）、`:5-18`（五步流程图：登录四步+刷新第 5 步）、`:306-310`（authDeviceCode 表单仅 client_id/code_challenge/code_challenge_method）、`:380-383`（deviceCodeToToken 仅 uid/code_verifier） | 平台「应用类型」分类选项（如个人应用/工具类）以页面实际选项为准——**提交时按平台页面核对** |
| 3 | code_challenge 构造 / PKCE Code Challenge | `BASE64URL-NOPAD(SHA256(code_verifier))`；挑战固定 43 字符、无 `=` padding；RFC 7636 附录 B 官方向量测试通过 | `crates/cloudmount-auth/src/pkce.rs:3`（官方要求注释）、`:62-65`（challenge() 实现，`URL_SAFE_NO_PAD` @ :64）；测试 `:73-80`（RFC 7636 附录 B 官方向量）、`:89-94`（43 字符+NOPAD 断言） | — |
| 4 | code_challenge_method / Challenge Method | 代码字面值 **`"sha256"`**（小写）。注：RFC 7636 官方方法名为 "S256"，两者指同一算法（SHA-256+BASE64URL-NOPAD），语义等价已由测试向量证实；若平台表单要求字面 "S256"，人类提交时按平台口径填写，工程侧实现无需改 | `pkce.rs:4`（注释）、`:26`（method 字段声明）、`:49`（generate() 赋值）、`:57`（from_verifier() 赋值）；`architecture.md` 决策 #2 | 平台表单字面口径（"sha256" vs "S256"）——**提交时按平台页面核对**，人类现场定填写值 |
| 5 | code_verifier 长度与字符集 / PKCE Code Verifier | 官方要求 43–128 字符；本项目固定 **64** 字符；字符集 RFC 3986 unreserved（`A-Z a-z 0-9 - . _ ~`）；OS CSPRNG 随机生成+拒绝采样消除模偏置（拒绝边界 198=66×3）；明确拒绝 p115client 的 `"0"*64` 固定 verifier 偷懒默认值 | `pkce.rs:4`（43–128 官方要求）、`:14-15`（UNRESERVED 字符集常量）、`:17-18`（VERIFIER_LEN=64）、`:31-46`（CSPRNG+拒绝采样）、`:7-8`（拒绝 "0"*64） | — |
| 6 | token 刷新机制 / Token Refresh Policy | ①**单飞**（single-flight）：同一把锁合并并发刷新，网络往返持锁、双检命中共享他人结果 ②**5 分钟提前量**（REFRESH_MARGIN_SECS=300，剩余不足即刷）③refresh_token **单次轮换**（每次刷新返回新 rt 覆盖旧值）④**40140117**（刷新过频）退避不重试 ⑤**40140123–26**（token 失效）强刷后重试一次 | `auth/open.rs:39-40,54-56`（提前量）、`:178-200,218-245`（单飞）、`:23`（40140117 退避）；`crates/cloudmount-driver115/src/client.rs:216-250`+`crates/cloudmount-core/src/envelope.rs:71`（40140123–26 失效重试） | — |
| 7 | scope/权限范围 / Scope · Permissions | **零 OAuth scope 请求参数**（全 crates grep：OAuth scope 零命中；`scope` 字面命中均为 std::thread::scope 标准库用法）——认证请求只发 client_id/code_challenge/code_challenge_method | grep 实测（OAuth scope 零命中；std 用法例证 `crates/cloudmount-sync/src/queue.rs:100-101`）；`auth/open.rs:306-310`（认证表单恰三字段） | 若平台表单有 scope/权限范围字段：**提交时按平台页面核对**，勾选范围以本表 #9 接口面为界 |
| 8 | redirect_uri/回调地址 / Redirect URI · Callback URL | **设备码流程无回调**——代码中无 redirect_uri 参数（grep 全 crates 零命中）；扫码确认后经 `deviceCodeToToken` 直接换 token | `auth/open.rs:5-18`（流程图无回调环节）；grep 实测（redirect_uri 全 crates 零命中） | 若平台表单强制要求填回调地址：**提交时按平台口径现场裁决**（开放问题③） |
| 9 | 所需接口面清单 / Required APIs | 15 端点（认证域 3+二维码域 1+资源域 11）——申请时如需申报 API 权限面，以此为准；明细见下方「接口面清单（15 端点）」 | 下方端点表锚列（逐端点实测）；域名三件套 `auth/open.rs:33-37` | 平台 API 权限申报粒度/勾选项——**提交时按平台页面核对** |
| 10 | 请求编码 / Content-Type | 认证域统一 `application/x-www-form-urlencoded`；资源域列表/详情 GET、写操作 POST 实况亦为 **urlencoded**（send_form） | `auth/open.rs:24`（注释）、`:148-166`（post_form→send_form）；`crates/cloudmount-driver115/src/client.rs:137`（资源域 send_form 实况）。⚠ `crates/cloudmount-driver115/src/open.rs:5-17` 文件头表格标「multipart」与实现矛盾——未在案文档漂移候选（供 B 系文档批修订，非本批范围） | — |
| 11 | 二维码有效期 / QR Code Expiry | 115 未明确给出，工程按 **5 分钟（300s）**处理，失效后重新发起 | `auth/open.rs:327-328`（注释自证「按常见 5 分钟处理」+`expires_at: now+300`） | 平台侧实际有效期以实测为准（5 分钟=客户端工程假设，非平台承诺） |

### 接口面清单（15 端点——按代码实际调用点枚举，非文档转抄）

域名三件套：认证 `passportapi.115.com`／二维码 `qrcodeapi.115.com`／资源 `proapi.115.com`（`auth/open.rs:33-37`）。

| 域 | 方法 | 端点 | 用途 | 锚（均实测） |
|---|---|---|---|---|
| 认证（passportapi.115.com） | POST | `/open/authDeviceCode` | 发起设备码（client_id+code_challenge+code_challenge_method） | `auth/open.rs:302`（表单 `:306-310`） |
| 认证 | POST | `/open/deviceCodeToToken` | 扫码确认后换 token（uid+code_verifier） | `auth/open.rs:377`（表单 `:380-383`） |
| 认证 | POST | `/open/refreshToken` | 刷新 token（单飞+轮换） | `auth/open.rs:204` |
| 二维码（qrcodeapi.115.com） | GET | `/get/status/` | 长轮询扫码状态（state 0=失效；data.status 1=已扫待确认/2=已确认） | `auth/open.rs:341`（状态判定 `:356,372-376`） |
| 资源（proapi.115.com） | GET | `/open/ufile/files` | 列表（分页 1000/页+user_utime 排序）+变更探测 | `driver115/open.rs:219`（排序 `:225`；变更探测 `:424,430`；PAGE_LIMIT=1000 `:37-38`） |
| 资源 | GET | `/open/folder/get_info` | 目录详情 | `driver115/open.rs:251` |
| 资源 | POST | `/open/ufile/downurl` | 下载直链（pick_code） | `driver115/open.rs:264` |
| 资源 | POST | `/open/folder/add` | 新建目录 ⚠TODO(verify)：参数 pid/file_name 按官方文档推断，M0 实测确认 | `driver115/open.rs:276`（TODO `:274`；extract_id `:279-280`） |
| 资源 | POST | `/open/ufile/update` | 重命名 ⚠TODO(verify)：参数 file_id/file_name 待实测确认 | `driver115/open.rs:373`（TODO `:371`） |
| 资源 | POST | `/open/ufile/move` | 移动（参数已确认：file_ids+to_cid） | `driver115/open.rs:385`（注记 `:383`） |
| 资源 | POST | `/open/ufile/copy` | 复制（参数已确认：单数 file_id+pid；响应 ID 字段待实测） | `driver115/open.rs:395`（注记 `:392-397`；响应 `:398-402`） |
| 资源 | POST | `/open/ufile/delete` | 删除（参数已确认：file_ids 单字段逗号分隔） | `driver115/open.rs:409`（注记 `:406-410`） |
| 资源 | POST | `/open/upload/init` | 上传初始化（秒传判定：fileid/preid 大写 SHA1，preid=前 128 KiB） | `driver115/open.rs:324`（`driver115/src/hash.rs:14`） |
| 资源 | GET | `/open/upload/get_token` | OSS 上传临时凭证 | `driver115/open.rs:147` |
| 资源 | GET | `/open/user/info` | 用户/配额（rt_space_info 解析） | `driver115/open.rs:414` |

> 申报口径：只有实际调用的 15 端点才是申报权限面的真实依据（`architecture.md` 决策 #3——官方要求可能更新，文档转抄会引入未证实条款）；申请时不多勾（最小权限面）。

## 二、操作指引（人类执行步骤）

> 执行者：项目所有者（提交人）。工程侧零动作（intent.md:17）；平台要求以官方页面实测时点为准（intent.md:18）。

1. **访问 115 开放平台**：`https://open.115.com/`（申请入口注释锚 `crates/cloudmount-core/src/config.rs:30-31`——注释「115 开放平台 AppID（官方通道；申请 https://open.115.com/ ）」@ :30+`app_id` 字段 @ :31；CLI 侧同源提示 `crates/cloudmount-cli/src/main.rs:147`「申请入口: https://open.115.com/ （个人开发者可申请，审核约 7+7 个工作日）」；`docs/ROADMAP.md:8`（M0 首项「申请 115 开放平台个人开发者+创建 PKCE 应用」））。
2. **注册个人开发者+实名认证**：实名信息**人类线下持有，本清单不落明文**（开放问题①，`intent.md:23`；`prd.md:48`；AC2②——只留占位）。资质材料格式（证件照片等）按平台页面要求现场准备——**提交时按平台页面核对**。
3. **创建 PKCE 应用**：按第一节 11 项逐项填写——
   - 确定项（#2 应用类型/#3 challenge 构造/#5 verifier/#6 刷新机制/#10 请求编码/#11 二维码有效期，及 #9 接口面）按「技术值」列口径填写；
   - 未知项（#1 应用名称终稿/#4 method 字面口径/#7 scope 勾选/#8 回调地址/#9 平台申报粒度）按「未知项标注」列处理：以平台页面实际选项为准；
   - 平台表单出现代码事实未覆盖的强制字段（如强制回调地址/应用域名）→ 按第三节「现场裁决项」处理；
   - 应用宿主描述示例措辞=「宿主 App（M5+，技术未定）」——macOS 宿主不预写技术栈（`prd.md:25` AC1⑤；`docs/ARCHITECTURE.md:47` 同口径图示）。
4. **提交后记录**：受理号/应用标识/回执关键信息（提交日期、平台单号、回执要点）→ 回报工程侧落 `docs/aidlc/b2-115-qualify/tracking.md` 首条记录（status=submitted，U2 承接——含 check_method[平台页面入口+查看凭据]）。AppID 下发后：AppID 值（非密钥）可记 tracking.md notes；工程接入（config 注入/TODO(verify) 实测）属下游「AppID 实测」intent（`docs/ROADMAP.md:16`），本批不执行。
5. **敏感边界提醒**：提交材料与仓库留痕（本清单+tracking.md）不含任何密钥/凭证明文/实名证件信息（`intent.md:19`；AC2②）——本应用类型无 AppSecret（第一节 #2）；access_token/refresh_token 等运行时凭据仅落系统安全存储（SecretStore——`auth/open.rs:82,102-110`），不落文档；verifier 仅存内存（`PendingLogin` @ `auth/open.rs:60-69`，非 Serialize、不落盘），换 token 后即弃。

## 三、开放问题现场裁决项（人类提交时定）

> 编号沿用 `plan.md` 缺口节口径（prd.md 开放问题节未裁决项顺次）：①申请主体实名信息②受理超期升级口径③平台强制字段④应用展示信息。

- **③ 平台强制字段**（`prd.md:51`）：平台表单若出现代码事实未覆盖的强制字段（例如强制要求回调地址/应用域名，而设备码流程无 redirect_uri——见第一节 #8）：按「**无回调，设备码流程**」口径现场填报，还是向平台咨询？——人类提交时现场裁决（工程侧清单已按「无回调」标注）。
- **④ 应用展示信息**（`prd.md:52`）：应用名称（建议占位 "CloudMount"）、描述文案、图标——人类偏好裁决，工程侧不代拟终稿。
- 关联项（非提交时点裁决，仅索引）：① 实名信息=人类线下持有（操作指引步骤 2）；② 超期升级口径=`escalate_date`（提交+14 工作日）到期未果时触发人类裁决（催办/工单/换道），tracking.md notes 已留占位（`intent.md:24`；`prd.md:49`）。

## 附：锚与 grep 证据实测记录（落笔前复验——TP-01 佐证）

- **源文件全读（4）**：`crates/cloudmount-auth/src/pkce.rs`、`crates/cloudmount-auth/src/open.rs`、`crates/cloudmount-driver115/src/open.rs`、`crates/cloudmount-core/src/config.rs`——上文全部锚逐行核对。
- **辅助锚抽读（5 文件）**：`crates/cloudmount-driver115/src/client.rs:137,216-250`、`crates/cloudmount-core/src/envelope.rs:71`、`crates/cloudmount-driver115/src/hash.rs:14`、`crates/cloudmount-sync/src/queue.rs:100-101`、`crates/cloudmount-cli/src/main.rs:33-34,143-150`；文档锚 `docs/ROADMAP.md:3-4,8,16`、`docs/ARCHITECTURE.md:47`、`intent.md:17,18,19,23,24`。
- **grep 证据复验（3 组）**：
  - `grep -rn "redirect_uri" crates/` → **零命中**（第一节 #8 依据）；
  - `grep -rn "scope" crates/ --include="*.rs"` → OAuth scope **零命中**；字面命中均为 std::thread::scope 标准库用法（`thread::scope` 文本命中 4 处+闭包参数用法，例证 `sync/src/queue.rs:100-101`）（第一节 #7 依据）；
  - `grep -rn "TODO(verify)" crates/` → **恰 2 处**：`driver115/open.rs:274`（create_dir）、`:371`（rename）（接口面表 ⚠ 标注依据；U3 动态清点口径同源）。
- **蓝图锚勘误（1 处，已回报编排者）**：`architecture.md` 规格 #4 引 `pkce.rs:58`——实测 `method: "sha256"`（from_verifier）字面在 **:57**（:58 为闭括号行）；本清单按实测 :57 落锚。其余蓝图锚抽验（`pkce.rs:3,4,14-15,17-18,26,31-46,49,62-65,73-80`／`auth/open.rs:3,5-18,23,24,33-37,39-40,306-310,380-383`／`driver115/open.rs:274,371`／`config.rs:30-31` 等）与实文一致。
