# Gate#1 AI 互审 r2（2026-10-15）

> 评审对象：修订轮两工件（prd.md + test-strategy.md @ 68a58d2；gate1 复位 @ 64304b3）对照上轮发现（reviews/gate1-ai.md）与打回裁决（decisions.md 2026-10-15 GATE-REJECTED gate1）。
> 评审者：reviewer-pm（0/0/5）、reviewer-architect（0/0/5）、reviewer-qa（0/0/5）、reviewer-dev（0/1/8）——全部只读。
> 编排者去重合并：**0 BLOCKER / 1 MAJOR / 12 MINOR**。

## 上轮发现消解核对（复审重点 B1/M1/M2/M7——四方逐项实证，全部消解）

| 上轮 | 判定 | 证据 |
|---|---|---|
| B1（artifact 名零锚） | ✅ 消解 | T2/T6 各补「upload 步 artifact `name:` 以 `cloudmount-` 开头」——先例同型性实证（release-yaml.test.sh L176-177 锚 `name:` 字段值前缀非 `path:`）；pattern 收编链 name 环节自此有静态锚 |
| M1（AC3 拆解与 T11 冲突+既有产物名零守护） | ✅ 消解 | prd AC3 拆解与 T11 双向改「双保险」口径（新脚本承接既有产物名锚点：cloudmount-linux-*.tar.gz/windows-x64.zip/darwin-*.tar.gz/digests-* 与既有 job 名）；T10/T11/T13 自相矛盾消除 |
| M2（前置硬校验无锚+双 arch 兜底不成立） | ✅ 消解 | T6 upload path 钉双字面精确路径（不用 glob）；T2/T6 各补前置硬校验断言（MSI=.exe 存在性/WiX 天然报错；pkg=package-pkg-macos.sh staging 硬校验调用字面——先例 L80-86 实证） |
| M3（升级披露不实+版本取值） | ✅ 消解 | 非目标改「不设计不保证，Release notes 提示先卸载再装」；版本取 github.ref_name 钉死（prd L55+T3 断言） |
| M4（双 SOP+SOP 载体） | ✅ 消解 | SOP 载体=operations 新 runbook 显式取代 macos-release 发布流程节（prd AC5+语义对齐+缺口节三处一致；被取代对象 L11-L23 实证存在） |
| M5（T16 P1） | ✅ 消解 | 升 P0；SKIP=显式人工裁决记 decisions.md（含失败输出留痕）；随 Q2 改 WiX 冒烟 |
| M6（T20 降级口径） | ✅ 消解 | 降级判据钉死（xar! magic+体积带；真机补验触发入 SOP） |
| M7（路径悬空+脚本名冲突） | ✅ 消解 | packaging/wix/main.wxs、scripts/package-pkg-macos.sh、installers-yaml.test.sh（复数）双文件钉死、grep 零单数残留；T14 断言字面单一事实来源，红态断言可落笔 |
| M8（WiX v3 EOL 权衡） | ✅ 消解 | Q2 裁决=手写 .wxs+CI 直接 WiX（cargo-wix 退出为「不采用」语境）；v3 EOL 留痕；安装锁定留 Construction 落点 |

Q1-Q4 裁决值与钉死项五件在 prd/test-strategy 逐字一致；计数 P0=15/P1=6/P2=1 逐行实数核对一致；AC↔T 双向映射无缺锚；三锁定脚本本机实跑全绿（AC3 Given 基线实证）；范围纪律 PASS（修订全落点名清单内，连带三处为裁决直接推论）。

## MAJOR（1，修订新引入）

- **R2-MAJOR-1｜`package-pkg-macos.sh`「dry-run 可本地跑」语义边界未定义，沿先例表述不可迁移**［dev；arch MINOR-2 同根］
  - 证据：prd L54「沿 package-macos.sh 先例：staging 归集 + 前置硬校验 + **dry-run 可本地跑**」——但先例的 dry-run 是**全代码路径**（占位文件真跑 tar、产物落 dry-run/，Git Bash 可用故成立）；pkgbuild 为 macOS 独占（test-strategy 已知事实自认「无 mac 机/无 codesign/pkgbuild」），pkg 腿本地 dry-run 至多覆盖 staging+硬校验段，**打包段本地不可执行**——prd 未界定此边界，「沿先例」措辞暗示可全路径本地预检。
  - 连带：pkg 脚本（钉死交付物）无任何行为测试锚（先例有 package-macos.test.sh 9 用例；本期 T1-T22 无一运行该脚本）——pkg 腿 tag 前本地预验证价值悬空。
  - 修复：prd L54 补一句「dry-run 仅覆盖 staging+硬校验段，pkgbuild 调用段 mac 独占、本地不覆盖」——纯事实边界声明，无需人类设计决策。

## MINOR（12，去重合并）

1. AC4 新增安装侧提示三要素（新开终端/UAC/先卸载再装）未入 T12 断言枚举（prd 锚「关键词」悬空）［pm+qa+dev 三方同证］。
2. pkg 侧 `--version` 取 github.ref_name 无 AC2 测试点（设计双腿覆盖、断言单腿）［qa］。
3. 收编链中段（`pattern: cloudmount-*`/`merge-multiple: true`/`files: dist/*`）仍零静态锚（T11 承接清单可补三字面，成本一条 grep；风险低于 B1——需改既有内容、diff 可见）［qa］。
4. T9「continue-on-error 白名单收窄」措辞两读（防护 vs 倒置）——建议改「负断言限定于 build/package/upload 步（这些步不得 continue-on-error）」［qa］。
5. SOP 固化清单② 缺 T18 的管理员 shell 前置［qa］。
6. T6 精确路径消静默归因过度（upload v4 的 if-no-files-found 仅全落空触发；精确路径实际贡献=完全静默收窄为日志可见 warning；单 arch 缺失真正闭环=前置硬校验+静态命名断言+E2E）——措辞如实化防实现者误判兜底范围［arch］。
7. T1/T5「按 prd/gate1 裁决字面写死」与头部「runner pin 未被钉死」矛盾（gate1 钉死项不含 job 命名/Windows runner；prd AC2 已写死 macos-15——三处口径不一，按头部口径统一）［pm+arch+dev 三方同证］。
8. AC2「无 preinstall/postinstall 脚本」声明已落但零测试锚（pkg install 脚本为经典供应链向量，负 grep 成本极低）［pm+arch 同证］。
9. WiX 大版本（v3 EOL vs v4/v5）未钉——.wxs 语法版本随之悬空（v3 choco 直装 vs v4/v5 需 dotnet SDK——本机实测仅 runtime）；版本锁定虽留 Construction，但 .wxs 写法与 T16 前置依赖该选择［dev］。
10. T3 .wxs 断言字面不可自然 grep（WiX 惯用 `ProgramFilesFolder`+`Name="CloudMount"`/`Environment`+`System="yes"`，字面「Program Files\CloudMount」「Machine」不出现在 .wxs——断言 token 需钉）［dev］。
11. T2 前置硬校验「或 WiX 天然报错等价声明」OR 逃生口弱化静态断言（后一分支无字面可 grep；pkg 侧具体字面不对称）［dev］。
12. （上轮遗留未点名）元数据 owner/pkg identifier 未钉（Cargo.toml=zhonghaoxi vs release.yml=tintac-cn 将流入安装器元数据）；MSI job 构建来源未明示（重编译 vs 下载 artifact）；UTF-8 中文 description 流入 .wxs 风险未入首日验证［dev/arch/pm 遗留］。

## 结论

**0 BLOCKER / 1 MAJOR / 12 MINOR。** 上轮 B1+8 MAJOR 全部按裁决消解且证据在案（含先例字面与基线脚本实跑实证）；Q1-Q4 与钉死项逐字一致；计数与映射诚实；范围纪律 PASS。唯一 MAJOR 为修订新引入的措辞精度问题（dry-run 语义边界——纯事实声明，一句话修复，无需人类设计决策）；12 MINOR 均为断言枚举补全/措辞消解级别。

**建议人类审批关注点**：①R2-MAJOR-1 处置——通过并附准出回填（编排者按既定事实修正 prd 一句 + MINOR 1-11 授权随 Construction 首日回填，沿 gate2 准出回填先例）或打回第三轮轻量修订（只改该句+MINOR 措辞）；②MINOR 12 三项遗留（owner/构建来源/UTF-8）建议随 Construction 首日验证一并落。
