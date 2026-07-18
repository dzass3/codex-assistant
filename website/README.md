# Codex Assistant public site

Codex Assistant 的公开产品展示与 Windows 安装包下载站。页面提供脱敏的 Live Agents、Smart Routing 和主题管理交互演示；真实本机控制能力只存在于 Windows 桌面版。

## 本地运行

需要 Node.js `>=22.13.0`。

```powershell
npm install
npm run dev
npm run lint
npm test
```

`npm test` 会生成 Cloudflare Worker 兼容的 vinext 构建，并验证 SSR 产品内容、桌面/网页能力边界、安装包和模板清理状态。

## 托管

`.openai/hosting.json` 绑定 OpenAI Sites 项目。站点不使用数据库、对象存储、身份验证或运行时密钥。
