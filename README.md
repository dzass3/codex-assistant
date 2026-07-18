# Codex Agent Monitor

一个 Windows 独立窗口工具，用于实时查看 Codex 根任务与子代理实际使用的模型、推理强度、运行状态和模型漂移。

## 隐私边界

- 只读打开 `~/.codex/state_5.sqlite` 和数据库引用的 rollout 文件。
- 只解析代理关系、有效模型、推理强度和生命周期等白名单元数据。
- 不保留、不发送、不展示提示词、回复、推理内容、工具参数或工具输出。
- 不修改 Codex 状态库或 rollout 文件；集成测试会在读取前后校验文件指纹。
- 默认完全本地运行，无遥测、无账号、无网络服务。

## 模型判定

有效模型按以下优先级显示：

1. rollout 中最新的 `turn_context.model`（运行时确认）；
2. `state_5.sqlite` 中的线程模型；
3. 若只有创建子代理时的请求值，则明确标为“仅请求值”，不会伪装成实际模型。

当请求模型与运行确认模型不同时，界面会显示模型漂移。

## 开发与验证

需要 Node.js、Rust 和 Windows C++ 构建工具。

```powershell
npm install
npm run tauri dev
npm run check
npm run tauri build -- --bundles nsis
```

核心实现位于 `src-tauri/src/monitor/`，前端只接收经过显式字段映射的脱敏快照。

## 来源与许可

本项目基于 [PixelPaw-Labs/codex-trace](https://github.com/PixelPaw-Labs/codex-trace) 的 Tauri 桌面基础开发。详情见 `THIRD_PARTY_NOTICES.md`，许可见 `LICENSE`。
