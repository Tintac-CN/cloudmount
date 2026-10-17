# cloudmount-web-ui

边缘网关管理页（单宿主，由 `cloudmountd --serve` 托管）：

| 宿主 | 运行方式 | 后端通道 |
|---|---|---|
| 边缘网关（NAS/树莓派/无头设备） | `cloudmountd` 直接托管静态文件 | `POST /api/*` → cloudmountd REST |

桌面端已切换为 egui 纯 Rust 壳（apps/desktop），不使用本 web-ui。

[`src/platform.ts`](src/platform.ts) 为服务端/边缘专用 fetch 单路径
（`POST /api/{cmd}`；已移除宿主探测，`hostKind()` 恒 `"web"`），
UI 代码只调用 `call(cmd, args)`。

## 开发

```bash
npm install
npm run dev        # http://localhost:5173，/api 代理到本机 cloudmountd:8384
npm run build      # 产物 dist/（由 cloudmountd --web-ui-dir 指向静态托管：Docker 镜像 COPY 至 /web-ui）
```

## 约定

- 前后端命令契约见 [`src/api.ts`](src/api.ts)：命令契约由 cloudmountd 单方实现为
  `POST /api/{cmd}`（daemon webdav.rs `/api/*`），**命名与参数必须一致**（以 api.ts 为准）。
- 刻意不用框架（React/Vue）：当前 UI 极简（状态页 + 托盘设置页），
  零运行时依赖换来最小体积；待 M2 若引入复杂列表再评估 Svelte。
