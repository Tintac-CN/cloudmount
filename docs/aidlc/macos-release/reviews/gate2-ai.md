# Gate#2 AI 互审（2026-10-15）

> 评审对象：Construction 交付 diff（ce02c2e..HEAD @ 6f8220d）对照 plan.md/prd/test-strategy 与 gate1 裁决。
> 评审者：**verifier**（新鲜上下文终检，独立复跑六命令+TDD 纪律 git 史核验+逐 unit 归属，0/0/0+3 INFO）、reviewer-architect（0/0/2）、reviewer-qa（0/0/3）、reviewer-dev（0/1/5）、reviewer-pm（0/0/2）——全部只读。
> 编排者去重合并：**0 BLOCKER / 1 MAJOR / 10 MINOR**（verifier 3 INFO 并入）。

## 复跑验证（verifier 亲自跑，未采信任何转述）

- `bash scripts/package-macos.test.sh` **9/9 exit 0**；`bash scripts/release-yaml.test.sh` **22/22 exit 0**；`bash scripts/check-macos-yaml.test.sh` **13/13 exit 0**（各含 actionlint SKIP+WARN 降级，plan 授权条款）。
- `cargo test --workspace`：**26 目标 / 277 passed / 0 failed / exit 0**——与 testBaseline（277/277/0）逐数一致，**零回归**；`cargo fmt --all -- --check` exit 0；`cargo clippy --workspace --all-targets -- -D warnings` exit 0 零告警。
- TDD 纪律 git 史核验合格：三个红提交（caeb2c2/8c0012d/9da5065）各只含测试文件且彼时被测对象必红（ls-tree 实证）；三个绿提交（0805095/c310f77/75125c7）各不含测试文件改动；锁置位（546ee92/559a301/f1cff56）严格落在红绿之间且仅改 state.json；三测试文件锁后零修改（git log 各仅 1 提交）。
- 文件归属：`git diff --name-only ce02c2e..HEAD` 共 10 文件，全部归属 U1-U4+工件链，零归属外；unitStatus 4/4=done 与逐 unit diff 存在性一致；**state.json testBaseline 逐字节未动**；plan/units/decisions 施工期间零改写。
- 抽查：release.yml 全 diff 仅 3 hunk（mac job 纯插入/needs 行契约性追加/产物表+Gatekeeper 段纯插入），`on:` 逐字一致；check-macos.yml `on:` 键集={workflow_dispatch} 唯一；dry-run 产物不进上传路径独立成立；独立验收（临时目录子进程）AC2 正/负路径、AC3①/AC4/AC5 python 断言全过；交付文件 TODO/FIXME 零命中、无硬编码密钥新增。

## MAJOR（1）

- **M1｜`--arch` 为契约外扩展且零断言覆盖，CI 命名正确性自动化守护悬空**［dev］
  - 证据：units/1.md 用法行、units/2.md、prd/test-strategy/decisions 均无 `--arch`；CI 双 tarball 命名落在 release.yml:93 `--arch "${t%%-*}"` 与 package-macos.sh:57-75 校验上；三测试脚本无任何 `--arch` 断言（package-macos.test.sh 只驱动默认 aarch64 路径，`--arch x86_64` 与非法值拒绝分支 :74 均无用例）。
  - 失败形态：CI 若丢/错传 `--arch`，第二次迭代同名覆盖，产出含 x86_64 二进制的 `cloudmount-darwin-aarch64.tar.gz`，T9-T13 全绿放行，仅 T15 人工核验兜底。
  - **实现在案且正确**（release.yml:93 传参字面与脚本白名单一致、与产物表名严格一致——verifier/architect/pm 三方核验通过），定性为断言覆盖缺口而非交付缺陷。修复方向：unlock 仪式（留痕）后给 release-yaml.test.sh 补一条 `--arch "${t%%-*}"` 结构断言，或 package-macos.test.sh 补 `--arch x86_64` 用例（后者需解锁该文件）；一句话级。

## MINOR（10）

1. T3 字面"产物目录路径非法 → usage 报错"与实现（package-macos.sh:78 专用报错 exit 1 不带 usage）及测试（仅缺参 ×2）双向未对齐［arch+qa+dev 三方同证］。
2. check-macos.yml 无 `permissions:` 显式声明（沿用仓库默认 token 权限；与既有 job 惯例一致，属最小权限加固可选项）［arch］。
3. release-yaml.test.sh/check-macos-yaml.test.sh 的 job_block 提取为行启发式而非 YAML 解析（第二个同名 job 键不会被扫描；终极回退仅 grep 键配平）——脚本头注已自认，精度边界［qa］。
4. `--arch` 非法值分支（:74）与可执行位失败分支（:80，T2 只删文件未 chmod -x）无测试用例［qa——与 M1 同根的独立分支］。
5. tar 中途失败时截断的 .tar.gz 留在 OUT_DIR（staging 有 trap、产物无；CI 因 job 失败不上传，风险限本地）［dev］。
6. 头注预检指引漏 `chmod +x` 步骤——照做者用 touch 建占位会在 -x 校验碰壁（报错自解释，T8"可照做"打折）［dev］。
7. edge.md mac 小节缺登录前置引用——serve 无登录态会 bail"请先执行 cloudmount login"（登录步骤在全局快速开始，小节未引用，照做在启动步碰壁）［dev］。
8. 行尾风格债：release.yml 工作区 CRLF、check-macos.yml LF，仓库无 .gitattributes（测试脚本已剥 CR 防御，功能无碍）［dev］。
9. T16 签名措辞未按授权②模板收全（改了"无 Developer ID 签名的二进制"，缺"/公证（arm64 ad-hoc）"括注；T12/SOP 已含全称，语义不误导）［pm］。
10. units/3.md:14 红绿路径仍写"追加至 release-yaml.test.sh"，与 PLAN-CONFIRMED 订正（独立文件）矛盾——实现遵循订正版本，纯工件字面陈旧［pm］。

## verifier INFO（3）

- actionlint 本机不可用：降级（YAML 解析+结构断言+记缺口）为 plan 明文授权条款，缺口节留痕在案；22/22 的 22 即未跑 actionlint 分支的诚实账目。
- units 字面"rustup target add"实现为 `dtolnay/rust-toolchain@stable` + `targets:` 参数（rustup 封装，功能等效；T6 断言兼容两形态）。
- T7/T14-T17 为线上人工 E2E，本机不可验（缺口节 SOP 已固化含第 0 步 dispatch check 前置），非 construction 缺陷。

## 架构/语义符合性（reviewer-architect）

语义对齐小节 9 条逐条符合（macos-15 顺序步/`-p` 同构/rust-cache/短 arch 命名/脚本单一事实来源/needs 门控/四项硬校验先行/dry-run 隔离/edge.md 小节）；fail-fast 链路闭环（顺序步中途失败→upload 不执行→needs 不发布，无部分成功形态）；brownfield 纯追加核验（既有 job 逐行零删改、`on:` 零改动、风格同款、artifact 经 `cloudmount-*` pattern 收编）；安全无新增面（无 secrets、权限沿用）。8 MINOR 回填 8/8 落实（T16 差括注见 MINOR-9）。

## 结论

**0 BLOCKER / 1 MAJOR / 10 MINOR。** verifier 终态声称独立复跑全部属实（零编排者失实）；TDD 红绿纪律为可复核样例；AC1-AC5 静态面全部落点、E2E 未虚称；gate1 全部裁决（R2-MAJOR+8 MINOR）逐项落实；范围纪律 10/10 文件归属。唯一 MAJOR 为覆盖缺口（代码本身经三方核验正确），一句话级修复。

**建议人类审批关注点**：①M1 处置——通过并附准出回填（unlock 留痕→release-yaml.test.sh 补 `--arch` 断言→重锁，随批准条目授权）或打回轻量修订；②10 MINOR 处置（多数随准出回填顺带：T3 字面对齐/T16 括注/units/3.md 订正/头注 chmod +x/edge.md 登录前置；其余备案 P2：permissions 加固/启发式精度/tar 半成品/行尾风格）。
