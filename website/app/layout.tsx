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
  const title = "Codex Assistant — 原生代理路由与模型观察";
  const description = "观察 Codex 原生子代理的有效模型，并以质量优先的方式持续路由到 Terra、Luna 与 Spark。";
  return {
    metadataBase,
    title,
    description,
    icons: { icon: "/favicon.svg", shortcut: "/favicon.svg" },
    openGraph: { title, description, type: "website", images: [{ url: new URL("/og.png", metadataBase).href, width: 1200, height: 630, alt: "Codex Assistant 原生代理路由" }] },
    twitter: { card: "summary_large_image", title, description, images: [new URL("/og.png", metadataBase).href] },
  };
}

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="zh-CN"><body className={`${geistSans.variable} ${geistMono.variable}`}>{children}</body></html>;
}
