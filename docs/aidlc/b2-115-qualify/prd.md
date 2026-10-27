# PRD: b2-115-qualify

> 产出者：pm　日期：2026-10-18　来源：intent.md

> route=classic（decisions.md A1 协议缺省）。本批=**人类操作为主+工程侧零代码**（audit-findings §3.1 首行原判）——「测试」口径=清单完备性+跟踪口径在案+预备文档准确性（无代码测试）；AC 即可测交付项，qa 按编号映射。

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| U1 | 项目所有者（提交人） | 在 115 开放平台表单前拿到一份**从代码事实提取**的申请材料技术清单（应用类型/授权流程/token 机制/接口面/回调说明），技术项不凭记忆填答 | 表单技术描述一次填报准确，降低因描述错误被驳回的风险（外部审核约 7+7 工作日，驳回=双倍时延） | 期望结果①（intent.md:10）；约束「不猜测未证实条款」（intent.md:18） |
| U2 | 项目所有者（账号持有者） | 按清单完成个人开发者资质申请+PKCE 应用创建的**提交**，并把提交凭据（日期/单号/回执要点）留痕 | M0 唯一外部阻塞链正式启动（ROADMAP.md:8）；M0/M1 全部真实凭据验证链的起点 | 期望结果②（intent.md:10）；audit-findings §3.1 首行（B2=材料提交+受理跟进，人类操作为主） |
| U3 | 项目所有者（等待期管理者） | 有**在案的受理跟踪口径**：预期时点（7+7 工作日的起算与检查方式）、超期升级路径 | 等待期可计划（audit-findings §5.2：等待期与 B1/B3/B5 完全并行）；超期时不悬置 | 期望结果③（intent.md:10）；开放问题②挂出待裁决（intent.md:24） |
| U4 | 下游「AppID 实测」intent 执行者 | AppID 下发时拿到**逐条预备清单**（TODO(verify) 两处+config 注入点确认），零摸索进入实测 | 接入面零重新勘探；下游 intent 的输入现成 | 期望结果④（intent.md:10）；ROADMAP.md:16（待 AppID 实测=独立下游 intent，本批只预备不执行，intent.md:20） |

## 验收标准（可测）

- **AC1**（U1）**申请材料技术清单定稿**：Given 本 intent 已接入且工程侧零代码约束生效（intent.md:17），When 工程侧从代码事实提取 PKCE 应用创建所需技术参数形成清单，Then 清单逐项在案且**每项附代码锚**，缺任一项即不通过：
  - ① 应用类型=PKCE（**无需 AppSecret**）——锚 `crates/cloudmount-auth/src/open.rs:3`、`crates/cloudmount-auth/src/pkce.rs:1-8`（115 官方要求注释）；
  - ② `code_challenge_method="sha256"`（即 S256）——锚 `pkce.rs:4,49`；
  - ③ `code_challenge=BASE64URL-NOPAD(SHA256(code_verifier))`，挑战固定 43 字符——锚 `pkce.rs:62-65`、`pkce.rs:89-94`（含 RFC 7636 附录 B 官方测试向量 `pkce.rs:73-80`）；
  - ④ verifier=64 位 CSPRNG 随机串（115 要求 43–128）、RFC 3986 unreserved 字符集——锚 `pkce.rs:15,18,31-51`；
  - ⑤ 授权模式=设备码扫码四步（`authDeviceCode`→115 App 扫码→长轮询→`deviceCodeToToken`），请求表单字段仅 `client_id`/`code_challenge`/`code_challenge_method`，**无 redirect_uri/回调地址**——锚 `crates/cloudmount-auth/src/open.rs:5-18,300-312,332-409`；
  - ⑥ token 刷新机制=`refreshToken` 换发新 refresh_token+提前 300 秒刷新+单飞合并防 `40140117`（刷新过频）——锚 `open.rs:16-18,23,40,203-245`；
  - ⑦ 域名三件套=`passportapi.115.com`（认证）/`qrcodeapi.115.com`（扫码轮询）/`proapi.115.com`（资源）——锚 `open.rs:33-37`；
  - ⑧ 所需接口面（对应平台 scope 申请范围）：`ls`（分页+usertime 排序）、`quota`（rt_space_info）、`download`（downurl 直链+Range）、`upload`（upload/init 秒传/二次认证/OSS 直传，preid 前 128K）、写操作（folder/add、ufile/update、ufile/move；copy/delete 的响应字段按 ROADMAP.md:16 属待实测确认项）——锚 audit-findings §1.1 L12-L14 行+`crates/cloudmount-driver115/src/open.rs:264,276,324,373,385`；
  - ⑨ 平台侧未知字段（scope 官方名单、资质材料格式、表单强制项等）逐项标注「**提交时按平台页面核对**」，零猜测条款——约束 intent.md:18。
  - **And** 清单不含任何密钥/凭证明文（敏感边界 intent.md:19）。

- **AC2**（U2）**提交执行与凭据留痕**：Given AC1 清单已定稿，When 项目所有者在 115 开放平台完成个人开发者资质申请与 PKCE 应用创建的提交，Then ①提交凭据在案：提交日期、平台单号/应用标识（如平台返回）、回执关键信息，记录于本批跟踪文档；②个人实名信息**只留占位不落明文**（开放问题①——人类线下持有，intent.md:23）；③产品代码零变更（`git diff` 无 `crates/`、`apps/` 及产品配置改动——约束 intent.md:17）。

- **AC3**（U3）**受理跟踪口径在案**：Given AC2 提交已发生（提交日=起算日），When 工程侧记录受理跟踪口径，Then 在案内容含三项：①预期时点=初审+复审约 **7+7 工作日**（来源 intent.md:6、audit-findings §3.1 首行）及各检查时点；②受理状态的查看方式在案（在哪个页面/以什么凭据查看）；③**超期升级路径**栏位在案——人类裁决前留占位并显式标注「待人类裁决（开放问题②）」，裁决后回填（intent.md:24）。

- **AC4**（U4）**AppID 到位预备清单**：Given AppID 尚未下发（外部审核中）且本 intent 不执行实测（见非目标），When 工程侧整理接入预备清单，Then 逐条在案：①TODO(verify) 之一=`crates/cloudmount-driver115/src/open.rs:274` create_dir `/open/folder/add` 参数名（pid/file_name）按官方文档推断，待实测确认；②TODO(verify) 之二=`crates/cloudmount-driver115/src/open.rs:371` rename `/open/ufile/update` 参数名（file_id/file_name）待实测确认；③对照锚=move_entry 参数**已确认**（file_ids/to_cid，`open.rs:383`）——作为「已确认/待确认」区分口径的示例；④AppID 注入点确认：`crates/cloudmount-core/src/config.rs:30-31` `app_id` 字段+环境变量 `CLOUDMOUNT_APP_ID`（config.rs:157-160）+优先级链 CLI flag>env>配置文件>默认（config.rs:16-18）+消费点 `OpenApiAuth::new(app_id, store)`（`crates/cloudmount-auth/src/open.rs:89`）与 `client_id` 表单字段（open.rs:307）；⑤AppID 到位动作=写入 config.toml（路径见 config.rs:6-12）或设 `CLOUDMOUNT_APP_ID` 后执行 `login`——并标注「本 intent 不执行，由下游 intent 承接（ROADMAP.md:16，含 copy/新建目录响应字段确认）」。

## 非目标（明确排除）

- **零代码变更**（首位约束）：不改任何产品代码/产品配置/CI——工程侧产出仅为清单/预备文档类（intent.md:17；audit-findings §3.1 原判沿袭）。
- **不做 AppID 实测**：真实账号跑通全链路+TODO(verify) 闭环=独立下游 intent（ROADMAP.md:16；intent.md:20）——本批只预备不执行。
- **不做 Cookie 通道相关任何申请**：Cookie 通道已独立完成且不等 AppID（ROADMAP M3「`--channel cookie` 挂载读取不等 AppID 即可用」，ROADMAP.md:151）。
- **不猜测 115 平台审核标准与未证实条款**：平台要求以官方页面实测时点为准，未知项标「提交时按平台页面核对」（intent.md:18）。
- **材料不含密钥/凭证明文**：.env/密钥类零触碰，guard-paths 照常（intent.md:19）。

## 开放问题（需人类裁决）

- [ ] 申请主体信息（intent.md:23）：个人开发者实名信息人类线下持有——工程侧清单仅留占位，提交时由人类自行填写，不落仓库。
- [ ] 受理超期升级口径（intent.md:24）：7+7 工作日到期未果→催办/换道/并行推进？裁决后回填 AC3 ③栏。
- [ ] （新增）受理跟踪留痕载体：decisions.md 为只追加（guard-gates deny Edit），而受理状态需多次更新——建议在本批目录内新建独立跟踪文档（如 `tracking.md`）承载 AC2/AC3 留痕，是否接受？
- [ ] （新增）平台表单若出现代码事实未覆盖的强制字段（例如强制要求回调地址/应用域名，而设备码流程无 redirect_uri）：按「无回调，设备码流程」口径现场填报，还是向平台咨询？需人类提交时现场裁决（工程侧清单已按「无回调」标注，AC1 ⑤）。
- [ ] （新增）应用对外展示信息：应用名称（建议占位 "CloudMount"）、描述文案、图标——人类偏好裁决，工程侧不代拟终稿。
