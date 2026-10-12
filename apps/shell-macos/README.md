# CloudMount macOS 壳（File Provider 扩展）

> 状态：**占位**。计划于 Windows 版（M2）稳定后启动。

## 方案（来自 2026-10 调研）

- 用 **`NSFileProviderReplicatedExtension`**（macOS 11+）—— Apple 官方的
  OneDrive/Dropbox 同款机制：占位符（dataless file）、按需下载、Finder
  云状态图标、"立即下载/释放空间"右键，与 iCloud Drive 相同 UX。
- **扩展入口必须 Swift/Obj-C**（`NSExtensionPrincipalClass`），同步引擎
  不重写：`cloudmount-core` 编译为 **staticlib**（C ABI），Swift 薄桥调用
  （`cbindgen` / `swift-bridge`）。社区已有 `objc2-file-provider` 绑定可参考。
- 扩展进程**内存上限低、生命周期短**：重活（传输、缓存、DB）放宿主 App
  或独立 XPC 服务，扩展只做协议翻译。

## 硬性要求

- App Sandbox + `application-groups` entitlement（扩展与宿主共享容器）
- 分发：Mac App Store 或 Developer ID 公证（需付费 Apple Developer 账号）
- 不用 macFUSE/FUSE-T 做主方案：做不出占位符与 Finder 集成（仅作遗留备选）

## 目录（待建）

```
shell-macos/
  CloudMount.xcodeproj         # 宿主 App + FP 扩展 target
  FileProviderExt/             # Swift：NSFileProviderReplicatedExtension 薄桥
  RustCoreBridge/              # cbindgen 生成的 C 头 + staticlib 链接配置
```
