import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { headers } from "next/headers";
import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export async function generateMetadata(): Promise<Metadata> {
  const requestHeaders = await headers();
  const host = requestHeaders.get("x-forwarded-host") ?? requestHeaders.get("host") ?? "codex-assistant.site";
  const protocol = requestHeaders.get("x-forwarded-proto") === "http" ? "http" : "https";
  const metadataBase = new URL(`${protocol}://${host}`);
  const title = "Codex Assistant — 安全的一键 Codex 换肤";
  const description = "为 Windows 官方 Codex 一键应用版权已核验或本机导入的主题，不遮挡文字、图标与输入区，随时恢复官方外观。";
  return {
    metadataBase,
    title,
    description,
    icons: { icon: "/favicon.svg", shortcut: "/favicon.svg" },
    openGraph: { title, description, type: "website", images: [{ url: new URL("/og.png", metadataBase).href, width: 1200, height: 630, alt: "Codex Assistant 安全一键换肤" }] },
    twitter: { card: "summary_large_image", title, description, images: [new URL("/og.png", metadataBase).href] },
  };
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="zh-CN"><body className={`${geistSans.variable} ${geistMono.variable}`}>{children}</body></html>;
}
