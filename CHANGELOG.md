# Changelog

## 0.12.0 — 2026-07-30

- Added four redistribution-approved 1672×941 landscape themes—镰仓雨夜, 湘南落日, 长安烟火 and 江岛暮光—and expanded the offline public catalog from 12 to 16 bundled themes.
- Embedded every new runtime and preview asset in the Windows application and installer so the same catalog is available to all users without device-local imports.
- Kept active themes applied while ordinary project, conversation-project and other non-sensitive Codex dialogs are open; sensitive dialog evidence still restores the official appearance.
- Added stable reading surfaces for plan previews and expanded plan panels without changing official text, images, icons, controls or hit targets.
- Rebuilt the rights manifest and verified all 16 themes across 256 viewport/scale cases, three clean matrix runs and three required failing safety canaries.
- 新增镰仓雨夜、湘南落日、长安烟火和江岛暮光四套可公开分发的横版主题，内置离线主题库由 12 套扩展到 16 套，所有用户安装后均可直接使用。
- 修复添加项目、生成对话项目及普通项目弹窗打开时皮肤消失的问题；登录、授权、密码及其他敏感弹窗仍会安全恢复官方外观。

## 0.11.9 — 2026-07-30

- Replaced four bundled themes—海风晴夏, 云岭秋侠, 流星晚霞 and 富士秋光—with newly rights-reviewed 1672×941 landscape artwork while preserving their stable theme IDs, favorites and saved preferences.
- Retired 紫夜剑影 and 春日花街 from source assets, runtime bundles, previews and installer resources; the public catalog now contains exactly 12 bundled themes.
- Added fail-closed migration for preferences that reference retired themes: the invalid selection is cleared instead of silently applying a different theme.
- Rebuilt and verified the complete theme catalog, embedded Rust resources and multi-window mock matrix so every public installer receives the same offline theme set.
- 更新四套内置主题的横版原图，同时保留原主题 ID、收藏与偏好兼容性；紫夜剑影和春日花街已从公开目录及安装资源中退役。
- 内置主题目录调整为 12 套；无效的退役主题偏好会安全清除，不会静默替换成其他皮肤。

## 0.11.8 — 2026-07-28

- Rebuilt all 14 theme surfaces around one crisp fixed global background with focal-aware `cover` positioning and no full-page white wash.
- Unified the navigation, header, output panel and composer as dark glass chrome while keeping conversation text, native actions, images and content icons intact.
- Added bounded reading glass for the current Codex assistant-message, tool-call and file-result DOM, with terminal cascade overrides for the native composer and output-panel subheaders.
- Added a theme-owned empty new-task welcome with four bounded shortcuts; it disappears as soon as conversation evidence appears and never sends a message automatically.
- Extended the mock gate to cover the empty-home transition, safe native-composer prefilling, dark-glass tokens and the existing 224 viewport/zoom cases.

## 0.11.7 — 2026-07-28

- 修复选择或复制文字时官方页面创建临时浮层，主题兼容观察器误判主界面不可见并禁用背景的问题；临时选择/复制浮层不再触发主题回滚，登录、账户、支付、授权和安全页面仍保持官方外观。
- 海风晴夏、云岭秋侠、流星晚霞、紫夜剑影、富士秋光和春日花街六款竖幅主题从右侧视觉轨道调整为窗口正中完整等比显示，继续保持 `auto 100%`，不裁切、不拉伸、不重复、不模糊。
- 新增选择/复制浮层回归场景，并继续覆盖 14 套主题、4 类窗口和 100%–200% 缩放矩阵。

## 0.11.6 — 2026-07-27

- 修复海风晴夏、云岭秋侠、流星晚霞、紫夜剑影、富士秋光和春日花街六款竖幅主题被横向窗口的 `cover` 布局放大裁切，只能看到局部人物或场景的问题。
- 六款竖图改为完整等比、全高、右侧视觉轨道显示；空余窗口区域由主题色渐变自然延展，既不拉伸、不重复、不模糊原图，也不改变其余八款横幅主题的焦点构图。
- 将竖图完整性纳入主题应用后的计算样式验证，并覆盖 14 套主题、4 类窗口、100%–200% 缩放共 224 个场景，继续验证文字、内容图片、图标、按钮、菜单、输入、滚动和敏感页面安全边界。

## 0.11.5 — 2026-07-27

- 将 14 套内置主题与本机导入主题的背景提升为固定在整个官方 Codex 窗口底层的单一 `cover` 图层，贯穿左侧导航、顶部、主内容与右侧输出区；背景不接收事件、不随内容滚动，页面切换不重复创建图层。
- 将统一白色蒙层拆为左侧阅读轨、中央内容轨、右侧人物轨和底部输入轨四级独立渐变；图片本身不再设置统一透明度或高斯模糊，窄窗与超宽屏根据主题焦点元数据稳定调整人物位置。
- 统一左侧导航、顶部栏、用户消息、下拉面板、悬浮输入框与右侧输出栏的暖玫瑰玻璃材质、边框、圆角、阴影、滚动条和 160–180ms 状态过渡，同时保持官方文字、按钮填充、SVG、菜单尺寸与所有点击区域不变。
- 扩展主题真渲染门禁到 14 套皮肤 × 1920×1080、2560×1440、3440×1440、窗口化 × 100%–200% 缩放共 224 个场景，并验证导航、输出栏、输入、发送、菜单、对话框、下拉、焦点、滚动和路由切换。

## 0.11.4 — 2026-07-27

- 14 套内置图片主题统一改为 `contain` 完整等比显示，不再使用 `cover` 或按窗口高度裁剪；竖图完整靠右、横图完整居中，不同比例产生的余白由主题渐变自然补齐。
- 移除主内容层对原图的 `backdrop-filter` 模糊并降低定向遮罩，让原图更清晰可见，同时继续保留正文、图标、按钮和输入区的可读与可点击边界。
- 将换肤范围扩展到拉取请求、站点、已安排、插件及其他使用官方主应用壳层的页面；登录、账户、支付、权限与安全页面继续保持官方外观。
- 新增完整背景比例、非对话页换肤、未知页面关闭以及 14 主题 × 宽/窄窗口 × 100%–200% 缩放回归门禁。

## 0.11.3 — 2026-07-27

- 修复首次点击“应用主题”时官方 Codex 主页面瞬态重排触发 `PartialApplication`、虽已安全回滚却立即报失败的问题。
- 只对 DOM 未就绪与已回滚的部分应用执行有界重试；CDP、身份、多窗口和其他终态失败继续立即关闭。
- 保留 0.11.2 的 Electron 根背景级联修复，并新增产品调用链、首次点击和终态失败不重试的回归覆盖。
- 重做 14 套图片主题的构图系统：竖图改为右侧全高单层主视觉，横图使用阅读区定向渐隐；统一增强正文、侧栏、顶部和输入区的可读玻璃层。
- 主题选择卡改用完整等比前景和同图低亮模糊衬底，春日花街、紫夜剑影、云岭秋侠、海风晴夏等竖图不再被 16:9 `cover` 预览裁剪。

## 0.11.2 — 2026-07-27

- 修复官方 Codex 26.721.4979 的 Electron 根节点背景规则覆盖主题背景、导致应用后验证失败的问题。
- 新增与官方 `[data-codex-window-type="electron"]` 背景级联冲突一致的浏览器回归场景，并继续覆盖 14 套主题、主区、侧栏、输入区和交互安全矩阵。
- 增加受保护 PID 门禁的真机主题复现脚本，真实验证新开的官方 Codex 窗口时不会连接或重启当前工作窗口。

## 0.11.0 — 2026-07-22

- 恢复只读“实时代理”页面，与“一键换肤”组成仅有的两个产品页面；Smart Routing、代理控制、路由 IPC 与打包资源仍保持移除。
- 代理树默认只显示启动中、运行中和跟踪异常任务并保留必要祖先，可切换查看全部；明确区分请求模型、已确认实际模型、来源、推理强度与模型漂移。
- 前端监控边界严格丢弃对话、推理、工具内容和完整路径；后端健康详情转换为固定脱敏文案。
- 主题重启门禁改用与可见观察器相同的活动数和置信状态；数据源降级或跟踪异常会阻止普通重启，强制票据绑定当前影响快照。
- 新增主任务、工具、敏感和未知页面分级；富主题只在可见且可点击的主任务/首页结构启用，登录、授权、权限、恢复、安全与未知页面保持官方外观。
- 扩展 Mock 兼容矩阵到 12 套内置主题、真实本机导入主题、菜单、对话框、下拉、焦点、滚动、链接与页面切换；继续保证文字、内容图片、SVG 和主按钮语义不被覆盖。

## 0.10.0 — 2026-07-22

- 移除“Codex（主题版）”启动入口、短时启动器和相关环境契约；官方 ChatGPT/Codex 入口保持不变。
- 主题只在用户打开 Codex Assistant 并点击“应用主题”后生效；完全关闭并从官方入口重开后需要再次手动应用。
- 升级时仅在目标确认为当前 Codex Assistant 可执行文件时清理旧版主题快捷方式，不修改官方快捷方式、安装包或本地数据库。
- 普通启动与状态轮询不再执行旧路由清理或自动重新注入主题；不会读取、改写或删除用户的 `.codex` 配置、代理、MCP 或全局 Skills。
- 保留受验证的 Store 进程识别、显式重启确认、旧进程树完全退出与稳定 app-server 检查。

## 0.9.1 — 2026-07-22

- 修复普通主题重启只等待主窗口、未等待旧 Codex 子进程树退出的问题，避免新旧 app-server 同时访问 `.codex` SQLite 数据库。
- 冷启动前检查当前用户的官方 Codex 运行时残留；检测到未退出的 `ChatGPT.exe`、`codex.exe` 或 code-mode host 时关闭失败，不启动重叠实例。
- 主题会话必须确认同一个直属官方 `codex.exe` app-server 连续稳定存活后才能标记成功，不能再只凭 CDP 可连接误报已应用。
- 新增历史故障只读复现脚本与进程树、app-server 稳定性回归测试。

## 0.9.0 — 2026-07-21

- 新增本机环境预检，逐项检测 Windows、官方 Microsoft Store Codex、窗口数量、主题控制会话、主题启动入口与已保存偏好，并将失败转为可执行指引。
- 改用 Windows `IApplicationActivationManager` 通过固定 AppUserModelID 启动 Store 版 Codex，修复直接执行受保护 WindowsApps 路径时的“拒绝访问”。
- 安装器创建“Codex（主题版）”开始菜单入口；用户主动启动时应用已保存主题后立即退出，不驻留托盘、不注册登录自启、不监控或自动重启 Codex。
- 区分冷启动、已验证会话、普通入口启动和多窗口歧义；普通入口启动只提供显式重启，失败时关闭失败且不误报已应用。
- 重启安全门禁改为统计全部正在启动或运行的原生任务，避免只统计子代理而漏掉根任务。
- 修复高 DPI 下首次窗口可能超出屏幕的问题，并升级公开网站与 Windows 安装包至 0.9.0。

## 0.8.0 — 2026-07-21

- 将产品收敛为专用的一键换肤工具，移除 Live Agents、Smart Routing、模型路由、预检、注入式路由控件和相关 IPC/打包资源。
- 新增独立的主题应用层；启动会话、应用、重试、恢复、本机图片导入和强制重启票据不再依赖路由状态或路由清单。
- 重写主题 CSS 安全边界：不覆盖 Codex 语义前景色、主按钮颜色或图标填充，不创建挡住内容的伪元素层，并验证主内容、侧栏和输入区仍可见且可点击。
- 主题切换采用先验证后提交的事务语义；兼容性或多页面应用失败时回退官方外观，不把局部成功误报为已应用。
- 升级时只迁移主题偏好和本机主题，并清理 Codex Assistant 以前安装的路由配置、代理文件与 MCP 项；用户自己的 Codex 配置保持不变。
- Windows 安装包和公开网站统一升级到 0.8.0，网站只展示一键换肤、本机导入、官方外观恢复和安全边界。

## 0.7.3 — 2026-07-20

- 将 Smart Routing 明确定义为原生 Subagent 编排模式：复杂且可拆分任务必须创建 2–4 个真实原生子代理，主 AI 保留协调、复核和最终决定。
- 把 Composer 下方的独立状态条移动到原生输入工具栏，隐藏内部路由 UUID，并加入可展开的子代理状态详情。
- 移除输入区的双层边框与双层阴影，增强顶部玻璃层、侧栏景深、内容卡片和聚焦态质感。
- 为控制层加入版本化热替换、受限元数据事件队列与兼容状态去重，避免短时 CDP 监听窗口丢失用户操作或旧 runtime 阻塞升级。
- 从净化后的路由状态同步子代理数量、阶段和有效模型；历史终态自动回到空闲，子任务降级不再永久禁用整个模式。
- 提高 Composer、顶部与侧栏主题覆盖的选择器特异性，确保 12 款内置主题和本机 Arina 都能稳定呈现圆角、分隔线和多层阴影。
- 路由资源版本升级至 2，确保现有安装能够获取新的强制编排契约。

## 0.7.2 — 2026-07-20

- Rebalanced image themes so the real backdrop stays visibly present through the main task surface while the sidebar and composer retain stronger readable glass layers.
- Promoted Aurora Grid's rights-cleared raster artwork from preview-only to its verified runtime main visual, so all bundled themes now use a full-resolution backdrop.
- Added palette-driven glass hierarchy for active sidebar rows, user message cards, headers, token surfaces, borders, focus states, scrollbar accents, and the composer fade without changing Codex's native layout.
- Strengthened image-theme verification and regression coverage so a heavily washed-out backdrop can no longer be reported as successfully applied.
- Reduced main-canvas backdrop blur to `2px` and raised verified image visibility to at least `40%`, while keeping the sidebar and composer on stronger readable glass layers.
- Preserved host token semantics for interactive controls and added contrast-safe primary-action foreground selection, including a mid-tone accent regression case.
- Aligned the local image limit with the encoded injection budget so every accepted local theme can still be applied within the bounded `2 MiB` control payload.

## 0.7.1 — 2026-07-19

- 修复图片主题的 CSS 数据 URL 转义与假阳性验证，只有图片 URL、计算样式和可读文字颜色都真实生效时才报告应用成功。
- 新增严格校验、仅保存在应用数据目录的本机主题目录；本机素材不会进入源码、安装包或发布产物。
- Smart Routing 改为只核对 `config.toml` 中的自有配置项，Codex 或用户对无关配置的修改不再被误报为冲突；恢复时保留无关修改。
- 配置冲突、未完成预检和其他阻止原因会持续显示，未满足前置条件时根任务开关明确禁用。
- 监控边界不再读取任务标题或保留 `spawn_agent.task_name`/`agent_path`，根任务与子代理使用项目名、系统昵称和不透明 ID 生成安全标签。
- 适配 Codex 26.715 当前的活动任务标记与编辑器结构，Smart Routing 控制可重新绑定到真实主任务输入框。
- 原生能力预检现在允许同一页面依次插入不同验证指令，并从每条指令实际插入时开始独立计算超时。
- 当 rollout 未提供 `requested_model` 时，只从四个白名单 `codex_assistant_*` 原生 profile 精确映射请求模型，并继续以 `turn_context` 有效模型和真实父子层级完成验证。

## 0.7.0 — 2026-07-19

- 在每个根任务行增加独立 Smart Routing 开关，并将同一状态绑定到对应 Codex 主任务输入框。
- 新增“正常、等待打开、下一条消息、已启用、需要修复”激活状态；关闭只影响后续任务，不中断运行中的子代理。
- 原生输入框标记通过精确插入结果确认后才显示为已启用，页面切换时自动重绑定且不会切换用户窗口。
- 主题应用在重启后等待主任务页面就绪，并在同一次操作中验证计算样式确实可见；工具窗口不再阻止主任务主题。
- 主题选择持久化为暂停状态，恢复官方外观始终提供明确反馈，并与 Smart Routing 配置相互独立。
- 节省信息仅在存在真实样本时展示；数据不足时明确说明，不复制真实任务制造对照数据。

## 0.6.0 — 2026-07-19

- 将重启、主题会话和一键应用统一到串行生命周期协调器；安全重启仍为默认模式。
- 活跃子代理阻止重启时，可通过 60 秒单次票据明确确认五秒宽限期后的受控强制重启，并在 PID、身份或影响变化时关闭失败。
- 主题状态区分已选择与已验证应用，持久会话和 early-script 在使用前重新校验，失败不会误报成功。
- 内置主题扩充为 12 个项目原创、哈希锁定且人工权利复核的可再分发主题。
- 原创主题预览改用紧凑 WebP 资源，在保留完整视觉效果的同时将公网安装包控制在静态托管限制以内。
- 更新 Tauri IPC/ACL、可访问确认交互、错误码、安装包和公开下载页。

## 0.5.0 — 2026-07-18

- 产品更名为 Codex Assistant。
- 保留既有升级标识与设置目录，兼容现有安装和本地配置。
- Live Agents Observer 仍保持只读的元数据观察能力。

## 0.4.0 — 2026-07-18

- 首个 Codex Agent Monitor 版本。
- 实时显示根任务、子代理、有效模型、推理强度和生命周期。
- 增加请求模型与实际模型漂移提示。
- 采用只读 SQLite 与 rollout 白名单解析，移除会话内容查看能力。
- 提供 Windows NSIS 安装包。
