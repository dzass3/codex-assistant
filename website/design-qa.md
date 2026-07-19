# Codex Assistant Web 首页设计 QA

- source visual truth path: `C:\Users\EDY\.codex\generated_images\019f6e2d-d2f8-7721-a7d6-b2f948dafd23\exec-546607a2-c79a-44e4-b773-4c46465b4c19.png`
- implementation screenshot path: `C:\Users\EDY\AppData\Local\Temp\codex-assistant-option2-1440x1024-pass4.png`
- viewport: `1440 × 1024`（主对照）；另检查 `1280 × 720`、`1366 × 768`、`1440 × 900`、`1600 × 900`、`1680 × 1050`、`1920 × 1080`
- state: 深色默认主题、首页首屏、实时代理标签页默认选中
- full-view comparison evidence: `C:\Users\EDY\AppData\Local\Temp\codex-assistant-design-qa-full-comparison.png`（左：选定设计；右：实现）
- focused region comparison evidence: `C:\Users\EDY\AppData\Local\Temp\codex-assistant-design-qa-demo-comparison.png`（左：选定设计中的产品窗口；右：浏览器实现）

## Findings

- 没有剩余的 P0、P1 或 P2 问题。
- 选定设计为了在一张概念图中展示更多内容，将产品窗口压进了首个视口；实现依据用户硬性约束保留 `100svh` 首屏，使导航、核心文案、轨道和四项指标在 `1366 × 768` 与 `1280 × 720` 下完整可见。该差异是有意的产品约束，不是设计漂移。
- 实现中的产品窗口比概念图更高，以保证 requested/effective model、状态和标签文字清晰可读；信息层级、布局关系、玻璃表面、细边框与蓝青状态色保持一致。

## Required Fidelity Surfaces

- Fonts and typography: 使用现有 Geist / Geist Mono 与系统中文回退；标题的字重、负字距、三行层级和蓝青渐变与选定方向一致。正文为 13.5–15px、1.75–1.85 行高，无截断；桌面边界标题采用显式语义换行，避免单字孤行。
- Spacing and layout rhythm: 首屏采用约 52:48 双栏、1440–1460px 内容上限与动态安全边距；六档桌面尺寸均无横向滚动、重叠或裁切。指标为统一面板，产品窗口、原则区和下载区保持稳定节奏。
- Colors and visual tokens: 深蓝黑背景、低透明蓝/青/紫光、精细边框、局部玻璃和低强度阴影符合选定方向；正文与背景对比清晰，状态色不单独承担含义。
- Image quality and asset fidelity: 天文台背景为独立原创 WebP，约 33KB，2048×1152 构图，无文字、品牌或版权角色；页面中 object-fit 保持比例，未拉伸。现有 Observatory Muse 图像继续以原始比例展示。
- Copy and content: 原有导航、标题、说明、按钮、Sol/Terra/Luna/Spark、四项指标、三个演示标签、隐私边界、Windows 安装包、大小与 SHA-256 摘要全部保留。
- Icons and decorative marks: 保留现有品牌标记与状态圆点，没有新增会暗示虚假功能的图标或占位资源。
- States and interactions: 已验证导航锚点、滚动导航状态、实时代理/Smart Routing/主题管理三标签切换、安装包 href/download 属性、按钮 hover/active/focus、模型节点联动、轨道与产品窗口轻视差、一次性进入动画。
- Accessibility: 保留语义 header/nav/main/section/footer、标签页角色与 aria-selected；键盘焦点可见；支持 `prefers-reduced-motion`、`prefers-reduced-transparency` 与高对比偏好。
- Performance: 持续动画以 transform/opacity 为主；pointermove 通过 requestAnimationFrame 节流且不触发 React 状态；页面隐藏时暂停 CSS 动画；未引入动画库；浏览器控制台无 error 或 warning。

## Comparison History

### Pass 1 — blocked

- [P0] 本地浏览器预览中，首屏 Next Image 走 Vinext 图像优化端点，但开发环境没有 ASSETS binding，出现错误覆盖层。
  - Fix: 首屏天文台背景与现有主题图使用 `unoptimized`，直接加载已优化的本地资源，不改变生产内容或业务逻辑。
  - Post-fix evidence: `C:\Users\EDY\AppData\Local\Temp\codex-assistant-option2-1440x1024-pass4.png` 无覆盖层，控制台无错误。
- [P2] 指标数字递增动画中的嵌套 span 继承了说明文字字号，数字在动画结束后仍偏小。
  - Fix: 为 `.metric-strip strong > span` 显式继承主数字颜色和字号。
  - Post-fix evidence: 1440、1366 与 1280 截图中 `2 / 4 / 1` 与指标主文本层级一致。
- [P2] 桌面边界标题在 1440px 下出现单字孤行。
  - Fix: 在不修改文案的前提下加入语义换行，使“控制。”作为完整短语进入第二行。
  - Post-fix evidence: `C:\Users\EDY\AppData\Local\Temp\codex-assistant-installer-1440x900-pass3.png`。

### Pass 2 — passed

- 主对照与聚焦对照均未发现新的 P0/P1/P2。
- 六档视口的 `scrollWidth` 均等于 `clientWidth`，首屏底部指标均处于可视区域。
- 主导航、演示标签与下载入口验证通过；控制台 error/warning 数量为 0。

## Open Questions

- 无。

## Implementation Checklist

- [x] 保留全部原文案、模型名称、模块与业务边界。
- [x] 保留并强化真实 Windows 安装包下载入口。
- [x] 完成 1280–1920 桌面布局与无溢出检查。
- [x] 完成加载、滚动、hover、视差、轨道和 reduced-motion 状态。
- [x] 完成产品标签页、导航和下载链接浏览器验收。
- [x] 修复全部 P0/P1/P2 设计 QA 问题。

## Follow-up Polish

- 无必须项。

final result: passed
