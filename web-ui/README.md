# cloudmount-web-ui

共享前端 SPA。**同一套代码**跑在两种宿主里：

| 宿主 | 运行方式 | 后端通道 |
|---|---|---|
| Tauri 桌面壳（Win/mac/Linux） | 系统 WebView 内嵌 | `invoke()` → 同进程 Rust 命令 |
| 边缘网关（NAS/树莓派/无头设备） | `cloudmountd` 直接托管静态文件 | `POST /api/*` → cloudmountd REST |

切换逻辑在 [`src/platform.ts`](src/platform.ts)：运行时探测 `__TAURI_INTERNALS__`，
UI 代码只调用 `call(cmd, args)`，对宿主无感知。

## 开发

```bash
npm install
npm run dev        # http://localhost:5173，/api 代理到本机 cloudmountd:8384
npm run build      # 产物 dist/（Tauri 与 cloudmountd 共用）
```

## 约定

- 前后端命令契约见 [`src/api.ts`](src/api.ts)：Tauri 侧实现为 `#[tauri::command]`，
  cloudmountd 侧实现为 `POST /api/{cmd}`，**命名与参数必须一致**。
- 刻意不用框架（React/Vue）：当前 UI 极简（状态页 + 托盘设置页），
  零运行时依赖换来最小体积；待 M2 若引入复杂列表再评估 Svelte。
