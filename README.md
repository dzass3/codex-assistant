# Codex Assistant

一个 Windows 独立窗口工具，用于实时查看 Codex 根任务与原生子代理实际使用的模型、推理强度、运行状态和模型漂移。

当前版本包括三项完整能力：

- Live Agents：只读观察根任务与原生子代理的请求模型、有效模型、层级和生命周期。
- Smart Routing：经过可见预检后，按质量底线持续将复杂任务分配给 Sol/Terra，将边界清晰任务分配给 Luna/Spark；支持 Terra → Luna/Spark 两层原生委派。
- 主题管理：在经过验证的本机 Codex 会话中一键应用 12 个版权已核验的声明式主题，并可恢复官方外观。

## 隐私边界

- 只读打开 `~/.codex/state_5.sqlite` 和数据库引用的 rollout 文件。
- 只解析代理关系、有效模型、推理强度和生命周期等白名单元数据。
- 不保留、不发送、不展示提示词、回复、推理内容、工具参数或工具输出。
- 不修改 Codex 状态库或 rollout 文件；集成测试会在读取前后校验文件指纹。
- 默认完全本地运行，无遥测、无账号、无网络服务。
- CDP 控制仅允许回环地址、同一 Windows 用户和官方 Microsoft Store Codex 进程；前端只开放列入 ACL 的窄 IPC 命令，不提供通用文件、任意 PID 终止或任意脚本接口。
- 安全重启不会终止活动子代理；用户明确确认的强制模式使用 60 秒单次票据、五秒宽限期和叶节点优先的身份复核终止流程。

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

公开展示站位于 `website/`。它包含交互式脱敏演示和 Windows 安装包下载；真实的本机观察、路由与换肤能力只在桌面版中运行。

## 来源与许可

本项目基于 [PixelPaw-Labs/codex-trace](https://github.com/PixelPaw-Labs/codex-trace) 的 Tauri 桌面基础开发。详情见 `THIRD_PARTY_NOTICES.md`，许可见 `LICENSE`。
