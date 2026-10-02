# 架构说明

> 应用源码位于 `YanRainOTAToolBox/` 目录，下文中的 `src/`、`src-tauri/`、`resources/` 均相对该目录。

## 总体分层

- 前端 `src/`：React 负责界面与交互，通过 Tauri `invoke` 调用后端命令。
- 后端 `src-tauri/`：Rust 负责设备交互、刷写、下载等能力。
- 资源 `resources/`：随应用分发的 ADB / Fastboot / scrcpy / aria2 等二进制。

## 目录职责

- `src/app/routes/`：每个功能一个页面目录。
- `src/app/components/`：可复用组件，按 `common` 与 `layout` 分组。
- `src/shared/`：`api` 封装后端调用，`store` 管理状态，`types` 定义类型，`i18n` 管理文案。
- `src-tauri/src/commands/`：暴露给前端的 Tauri 命令。
- `src-tauri/src/core/`：具体领域实现（adb、payload、scrcpy、boot、download）。
- `src-tauri/src/domain/`：纯领域模型，不依赖 I/O。
- `src-tauri/src/infra/`：配置、日志、HTTP 客户端、错误处理等基础设施。

## 数据流

前端路由页面 -> `shared/api` 类型化封装 -> Tauri `invoke` -> `commands` -> `core` 领域实现 -> 外部进程 / 设备。
