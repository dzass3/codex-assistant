# Codex Assistant website

Codex Assistant 0.11.9 的公开官网源码。站点介绍 12 套一键主题、本机环境检测、只读子代理观察、安全边界和本机图片导入，并为 x64/ARM64 提供 EXE/MSI 四种 GitHub Release 下载入口。

二进制安装包不提交到源码树或 Sites 源码仓库。页面中的版本、文件大小、SHA-256 和 GitHub Release 链接统一维护在 `app/release-data.ts`。

```powershell
npm ci
npm run release:verify
npm test
```

站点使用 Vinext/Sites 构建和发布。`.openai/hosting.json` 是本机 Sites 项目绑定，不进入公开仓库；没有该文件时公开源码仍可完成构建和测试。

0.11.9 安装包尚未代码签名，官网必须持续显示 SmartScreen/“未知发布者”提示与 SHA-256。
