# Gate#2 AI 互审（2026-10-16）

> 评审对象：intent `desktop-installers`（route=express）Construction 交付；diff 范围 `bbe706e..HEAD`（U1..U5 全部施工提交）。
> 评审方：reviewer-pm（0/0/7）+ reviewer-architect（0/0/5）+ reviewer-qa（0/1/4）+ reviewer-dev（0/2/8）+ verifier（0/0/2）——合并 **0 BLOCKER / 3 MAJOR / 26 MINOR**。

## verifier 终检结论（先置）

5/5 units 承诺交付物全部兑现且无弱化；全部验证声称（红态 58/53、111/111、9/23/13/72、基线 277/0、fmt/clippy/build、gen_icons 幂等、fail-fast 负路径）经独立复跑/repro 属实（含 U1 红态在 bbe706e 档案树的复现）；计划外文件双查=无未授权文件（`.gitignore`/build 与 `Cargo.lock` 均有 decisions.md 留痕，判定合理豁免）；四方对账（state↔磁盘↔git↔decisions）无实质差异；gate1 回执新鲜度成立。

## BLOCKER

无。

## MAJOR（3）

- **[reviewer-dev] M1 伪隐藏关闭状态机缺陷**：`apps/desktop/src/main.rs:223-241` hide 分支无「已隐藏」守卫——`restore_pos = Some(pos)` 无条件以当前 outer_rect 覆写。伪隐藏保持 WS_VISIBLE，窗口离屏后仍可持焦点再收 WM_CLOSE（Alt+F4/双击 X）→ 第二次 close_requested 把 restore_pos 毒化为 (-32000,-32000)，此后托盘「显示主窗口」复位到不可见位置永久失效（仅托盘退出可救）。Visible(false) 原契约形态天然免疫（隐藏窗收不到 WM_CLOSE）——此为伪隐藏偏差引入的新增风险面。修复≈一行守卫（`restore_pos.is_none()` 时才记录/隐藏态短路）。
- **[reviewer-qa] M2 TP-11 icns 子断言在首日验证③定稿后未落位**：test-strategy L35/L65 与脚本头注 L35-36 均承诺「icns 按首日验证③落位后断言」；decisions.md U2 已定稿=stdlib 预生成入仓且 `apps/desktop/icons/icon.icns` 已入仓——但 111 断言中**无 icon.icns 存在性断言**（TP-07/12 断的是脚本文本非产物文件），误删不报红。现有兜底两层（package-dmg-macos.sh L96 CI fail-fast + release-checklist §3 dry-run 判据），故非 BLOCKER；处置=补一行存在性断言（断言脚本已锁，需新 UNLOCK-TESTS 授权）或显式裁决豁免留痕。
- **[reviewer-dev] M3 U5 终验「fmt clean」证据未覆盖被改代码**：`cargo fmt --all` 不触 workspace exclude 的 apps/desktop；实测 `cd apps/desktop && cargo fmt --all -- --check` exit 1（394 行 diff，其中 main.rs:238-241 为本期新增行——超宽 tracing::info!）。基线本就 fmt-dirty（既有 theme.rs/widgets.rs ≈109 行债）。clippy 侧 desktop `-D warnings` 实测通过，缺口仅 fmt——处置=fmt 本期文件/全量 fmt apps/desktop/明示豁免+终验口径修正（workspace fmt 与 desktop fmt 分开声明）。

## MINOR（26，去重后按主题归并）

1. **release-checklist.md L21 证据指针笔误**（三方同报：pm-6/qa-3/dev-4/verifier-2）：「decisions.md『U3 应用层完成核收』」应为「U3 Windows MSI 扩展完成核收」——词级。
2. **release.yml:102-103 既有 job 头注失实未同步**（architect-1）：注释仍写「只铺 cloudmountd.exe + cloudmount.exe」，与同块 desktop 追加步矛盾——prd AC3④ 仅授权了 main.wxs L8-11 同类更新，此处漏同步。
3. **main.wxs:49-50 Package Description/Comments 仍为两件套描述**（pm-7/dev-1）：ARP 元数据欠准；不动符合 AC3④ 既有字面守护，修复需新授权，备案。
4. **units/2.md:19 契约字面无偏差指针**（architect-2）：仍为「CancelClose+Visible(false)」——授权链完整（decisions.md 留痕）但契约本体无指针。
5. **main.wxs:17 BinDir 注释行更新超 prd AC3④ 点名范围**（architect-3）：理据同「失实注记顺带更新」，注释行不计守护，程序性备案缺口。
6. **.gitignore /build/ 与 Cargo.lock 在改动面外**（pm-1/architect-4/dev-5）：均有 decisions.md 留痕（U3 施工注记/U2 授权 10 文件）——建议 gate2 追认。
7. **raw-window-handle 直声明**（pm-2）：gate1 r2 授权字面仅「winres 嵌 .ico」；服务伪隐藏取 HWND，decisions U2 遗留观察 b 已披露（原传递依赖 lock 实证）——建议 gate2 追认。
8. **AC5④「校验步两 exe→三 exe」就地改名**（pm-3）：严格读超 B2「允许追加」枚举，但为 A3 批准的 units/3.md L16 契约+TP-03 护栏在位——备案。
9. **Info.plist 版本注入超 units/4.md 键集枚举**（pm-4）：与 M3 安装器版本同源口径一致、decisions U4 披露——边缘扩面备案。
10. **伪隐藏物理形态偏差待追认**（pm-5/verifier-1/architect-5 部分）：语义一致+留痕完备，mac 侧记录口径窄于物理面（Mission Control 可见性未记）——gate2 人类确认点。
11. **HKCU KeyPath 多用户卸载孤儿未留痕**（dev-2）：经典 ICE43 代价，U3 同用户装卸未覆盖。
12. **dmg 脚本不校验二进制架构**（dev-3）：仅 [ -x ]，直跑场景无 file/lipo 兜底（CI 步序兜住）。
13. **TP-23 无显式承接载体**（qa-1）：U3 冒烟做了等价验证（MajorUpgrade 覆盖+卸载全清）但未标 TP-23；TP-24 人工核对半边同样未入清单（静态 3 断言已盖 body 字面）。
14. **T4 修订断言①防空真依赖步骤名字面**（qa-2）：`steps_matching 'daemon [+] cli'` 步骤名被改则提取空→结构脆弱（现态 72/72 实测+注入证据非空真）。
15. **「111 断言」为环境相关数**（qa-4）：actionlint 可用时为 112——口径注记建议。
16. **gen_icons 幂等跨 zlib 版本边界**（dev-6）：同环境幂等已实测，跨版本字节可能漂移（write-if-changed 如实重写）——注记建议。
17. **TP-12 build.rs 断言 'winres' 子串宽松**（dev-7）：同时命中 winresource——现态无害。
18. **FFI 32 位约束无注释**（dev-8/architect 观察）：GetWindowLongPtrW 64 位 API，本仓 CI 全 64 位成立——注释留痕建议。
19. **`.gitignore` L13-14 残留 src-tauri 死路径**（pm-1b）：「全仓 src-tauri 引用清零」留痕表述过强（AC1② 字面「图标路径引用」仍成立）。

## 结论

0 BLOCKER / 3 MAJOR / 26 MINOR。verifier 判定 5/5 units 兑现、验证声称全部复现属实、无计划外文件、四方对账无差异。3 个 MAJOR 均可快修（M1 一行守卫；M2 一行断言[需解锁授权]或豁免裁决；M3 fmt 口径三选一）——建议人类审批关注点：①总裁决（通过+准出回填 / 打回修订 / 直接通过备案）②icns 断言处置（补断言需新解锁授权 vs 豁免留痕）③fmt 口径（本期文件 vs 全量 vs 豁免）④追认项打包（.gitignore/Cargo.lock/raw-window-handle/伪隐藏偏差/校验步三 exe/plist 版本注入）。
