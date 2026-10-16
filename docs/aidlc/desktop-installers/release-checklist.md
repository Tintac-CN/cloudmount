# Release 前人工核验清单（TP-22 载体）

> **语义**：首个含 desktop 交付的 tag 即**直接正式发布**（无预发环境，prd A1/gate1 裁决）。
> **铁律**：本清单未逐项通过，**不打 tag**；任一项 FAIL → 修复后重验，不得带病发布。
> 适用范围：自 desktop-installers intent 起的所有 `v*` tag。清单为「打 tag 前」操作规程——
> 复选框在对应项**实际执行并通过后**勾选（附日期/证据），未执行项保持未勾选。
> 载体依据：test-strategy.md TP-22（「人工核验清单固化于仓库内工件 docs/aidlc/desktop-installers/ 附录文件」）。

---

## 0. 发布入口与环境前置

- tag 形态：`v<数值>`（如 `v0.3.0`）——**纯数值后缀**（WiX/pkg 脚本拒收 rc 形态，prd AC5）。
- push tag 即触发 `.github/workflows/release.yml`（`on: push: tags: v*`），无预发、无 dry-run 发布。
- 本机环境（Windows 冒烟面）：Git Bash；WiX v3.14.x 在 `/c/Program Files (x86)/WiX Toolset v3.14/bin`；**管理员 shell**（msiexec per-machine 静默装卸需提权）。

---

## 1. 本地 MSI 冒烟绿（TP-13/14/15/16/17）

- [ ] **状态：已过**（2026-10-16 U3 本地冒烟全过——证据：decisions.md「U3 应用层完成核收」前条目 + 本机 `build/` 日志[不入仓]；发布前需按下述命令**重跑一遍**确认当前 HEAD 仍绿）

核验方法（管理员 Git Bash，仓库根）：

```bash
cargo build --release -p cloudmount-daemon -p cloudmount-cli
cargo build --release --manifest-path apps/desktop/Cargo.toml
cp apps/desktop/target/release/cloudmount-desktop.exe target/release/
cp apps/desktop/icons/icon.ico target/release/
export PATH="$PATH:/c/Program Files (x86)/WiX Toolset v3.14/bin"
candle -arch x64 -dProductVersion=<tag 去 v> -dBinDir="$(pwd -W)/target/release" \
  packaging/wix/main.wxs -out build/main.wixobj          # 期望 exit 0、全量 ICE 零抑制
light build/main.wixobj -out build/cloudmount-windows-x64.msi   # 期望 exit 0
msiexec -i "build\\cloudmount-windows-x64.msi" -qn -norestart -l*v "build\\msi-install.log"
# 期望 exit 0（Git Bash 用 - 形式开关，避免 //l*v 转义坑）
```

通过判据：
- `C:\Program Files\CloudMount` **三件套**齐全：cloudmountd.exe + cloudmount.exe + cloudmount-desktop.exe（0.3.0+ 为当前构建）；
- 两处快捷方式在位且**目标 = cloudmount-desktop.exe**：`C:\ProgramData\...\Start Menu\Programs\CloudMount.lnk` 与 `C:\Users\Public\Desktop\CloudMount.lnk`（WScript.Shell 解析 TargetPath）；
- 安装后的 cloudmount-desktop.exe 含图标资源（PowerShell `EnumResourceNames`：RT_GROUP_ICON id=1 + RT_ICON≥1）；
- Machine PATH 含安装目录（`[Environment]::GetEnvironmentVariable('Path','Machine')`）；
- ARP `DisplayVersion` = 本次 tag 版本；
- `msiexec -x "build\\cloudmount-windows-x64.msi" -qn -norestart -l*v "build\\msi-uninstall.log"` exit 0，且卸载后**全清**：目录/两处快捷方式/Machine PATH 条目/HKCU+HKLM\Software\CloudMount/ARP 条目/无残留进程。
- 注意：冒烟会经 MajorUpgrade 覆盖本机已装旧版；冒烟结束建议保留卸载后（无 CloudMount）状态或重装所需版本——如实记录前后状态。

---

## 2. 断言脚本全绿，含零删改护栏（TP-03/TP-10；AC6）

- [ ] **状态：已过**（2026-10-16 U4 终验：五脚本 9/23/13/72/111 全 exit 0 + 基线 277/0；发布前需**重跑**）

核验方法（仓库根）：

```bash
for s in package-macos release-yaml check-macos-yaml installers-yaml desktop-installers-yaml; do
  bash scripts/$s.test.sh; echo "$s exit=$?"
done
cargo test --workspace        # 期望 277 passed / 0 failed（对照 state.testBaseline 逐数一致）
```

通过判据：
- 五脚本全 exit 0：package-macos **9/9**、release-yaml **23/23**、check-macos-yaml **13/13**、installers-yaml **72/72**（T4 为 gate1 B1 授权收窄形式：daemon/cli 编译步+upload 步与三既有铺放组件 desktop-free）、desktop-installers-yaml **111/111**（零删改护栏 TP-03 + 全部静态断言在内）；
- actionlint 本机不可用时脚本走降级分支（SKIP+WARN+yaml.safe_load，计数诚实）——有条件时在 CI/装 actionlint 复核整文件合法性；
- 任一 FAIL → 先修复被测面，**禁止**改锁定断言（除非走解锁→授权→修订→重锁流程）。

---

## 3. dmg 结构断言绿（TP-07；装配/真机 TP-19 另行）

- [x] **结构断言：绿**（desktop-installers-yaml 111/111 内含 TP-07 全部字面 + dmg job/upload/body 面；2026-10-16 U4）
- [ ] **状态（真机/CI）：待执行**——hdiutil 装配与 .dmg 真机行为无法在 Windows 本机验证，依赖本次发布 CI 的 `installer-dmg-macos`（macos-15）**首跑** + 真机（aarch64 mac）TP-19。

打 tag 前本地可验段（Windows 可跑）：

```bash
mkdir -p build/dryrun-bin
cp apps/desktop/target/release/cloudmount-desktop.exe build/dryrun-bin/cloudmount-desktop
chmod +x build/dryrun-bin/cloudmount-desktop
bash scripts/package-dmg-macos.sh --bin-dir build/dryrun-bin --out build \
  --version <tag 去 v> --dry-run        # 期望 exit 0（staging 三件就绪）
```

通过判据：
- dry-run exit 0（前置硬校验 binary/-x + icns 存在性 + bundle 布局三件）；缺 binary/icns → exit 1 且零产出（fail-fast 实测）；
- 打 tag 后（CI 段）：`installer-dmg-macos` job 绿、artifact `cloudmount-desktop-dmg` 含 `cloudmount-desktop.dmg`；
- 真机段（TP-19，发布后/发布人持有 aarch64 mac 时）：挂载 dmg → 拖 CloudMount.app 入 /Applications → 双击启动有界面 → icns 图标显示 → Gatekeeper 首启引导可用。

---

## 4. 托盘 + 双击 UI 人工冒烟通过（TP-20/TP-21）

- [ ] **状态：待人工**（Windows 侧「关闭=隐藏到托盘」已自动实证[U2 冒烟：离屏+TOOLWINDOW+进程存活、隐藏 5s CPU 0.016s]；**真实托盘点击与 mac 侧行为待真机人工**——托盘图标非可编程 HWND，无法自动化）

步骤（Windows，双击安装版 `C:\Program Files\CloudMount\cloudmount-desktop.exe` 或本地构建版）：
1. 托盘区出现 CloudMount 图标（gen_icons 品牌图标）；
2. 右键/左键菜单**恰两项**：「显示主窗口」「退出」——多一项即 FAIL（prd AC2② 封顶）；
3. 点「显示主窗口」→ 主窗口唤起/聚焦；
4. 点窗口关闭按钮（×）→ 窗口消失（任务栏无按钮）、进程仍在、托盘仍在（Q1：隐藏到托盘；实现形态=伪隐藏[离屏+去任务栏]，decisions.md 首日验证④偏差条目）；
5. 再点托盘「显示主窗口」→ 窗口复位可见；
6. 点托盘「退出」→ 进程结束 + 托盘图标消失（任务管理器核对无 cloudmount-desktop 进程）。

步骤（mac，真机 dmg 安装后）：同 1–6（Dock 图标留存为已知差异——decisions.md 偏差条目）。

通过判据：与 Q1 裁决完全一致（关闭=隐藏到托盘、托盘「退出」真退出、菜单恰两项）；双击 UI 冒烟（状态页/登录页渲染正常、无 panic）。任何不符 = FAIL，不得打 tag。

---

## 5. 产物清单与预期一致（TP-22/AC5⑤）

- [ ] **状态：待打 tag 后核验**（tag push → CI 8 job → Release 页；本项只能在 tag 后闭环，但 push 前**必须**复核本清单与 body 产物表一致）

预期 Release 附件（8 行表 / 9 文件）：

| # | 文件 | 来源 job |
|---|---|---|
| 1 | `cloudmount-linux-x86_64-unknown-linux-musl.tar.gz` | binaries-linux |
| 2 | `cloudmount-linux-aarch64-unknown-linux-musl.tar.gz` | binaries-linux |
| 3 | `cloudmount-windows-x64.zip` | binaries-windows |
| 4 | `cloudmount-darwin-aarch64.tar.gz` | binaries-macos |
| 5 | `cloudmount-darwin-x86_64.tar.gz` | binaries-macos |
| 6 | `cloudmount-windows-x64.msi`（三件套：daemon+cli+desktop） | installer-msi-windows |
| 7 | `cloudmount-darwin-aarch64.pkg` | installer-pkg-macos |
| 8 | `cloudmount-darwin-x86_64.pkg` | installer-pkg-macos |
| 9 | `cloudmount-desktop.dmg`（desktop .app，aarch64-only） | installer-dmg-macos |

（GHCR 镜像 `ghcr.io/tintac-cn/cloudmount:<tag>` 为镜像仓库产物，非 Release 附件。）

核验方法：push tag → 监视 Actions 全部 job（重点：`installer-msi-windows` 三件套、`installer-dmg-macos` **首跑**）→ Release 页核对附件集合与上表**精确一致**（无缺失/无多余/命名逐字）→ body 产物表 8 行正确（MSI 行三件套描述、.dmg 行含 Gatekeeper 引导）。

通过判据：9 文件齐全、命名逐字一致、body 表正确；**缺失/多余/命名漂移 = 不得发布**（删 tag 修复后重走全清单）。

---

## 变更记录

- 2026-10-16：U5 固化本清单（初版）。各项「状态」为固化时点如实记录——1/2 已过（本机可复跑），3 结构断言绿（真机/CI 待执行），4 待人工真机，5 待打 tag 后核验。发布人：逐项执行→勾选→全过方可 push tag。
