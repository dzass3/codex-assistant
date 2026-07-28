export const releaseVersion = "0.11.8";
export const releaseTagUrl =
  "https://github.com/dzass3/codex-assistant/releases/tag/v0.11.8";

const assetBase =
  "https://github.com/dzass3/codex-assistant/releases/download/v0.11.8";

export const releaseAssets = [
  {
    architecture: "x64",
    format: "EXE",
    recommended: true,
    fileName: "Codex Assistant_0.11.8_x64-setup.exe",
    bytes: 7_710_295,
    sha256: "63959AFCC716775DF2E418798A19BF15BFE915E57B835629473CEAE87FFFC256",
  },
  {
    architecture: "x64",
    format: "MSI",
    recommended: false,
    fileName: "Codex Assistant_0.11.8_x64_en-US.msi",
    bytes: 12_460_032,
    sha256: "DED11FBF67BF8D3E228B4767165A57144C5D7D16F83D84E851DF723902086F87",
  },
  {
    architecture: "ARM64",
    format: "EXE",
    recommended: true,
    fileName: "Codex Assistant_0.11.8_arm64-setup.exe",
    bytes: 7_426_966,
    sha256: "E7C2D52FEBEC29E22F5B5E8E59A2F76CE0BCA407A90567777CA24C1A6B9C79ED",
  },
  {
    architecture: "ARM64",
    format: "MSI",
    recommended: false,
    fileName: "Codex Assistant_0.11.8_arm64_en-US.msi",
    bytes: 12_308_480,
    sha256: "1FEEAA9A2BFF6FA6E36FF12D6AAE5E905EC4F4AF5A243992423D10EE2B6C2700",
  },
].map((asset) => ({
  ...asset,
  url: `${assetBase}/${encodeURIComponent(asset.fileName)}`,
  sizeMiB: `${(asset.bytes / 1_048_576).toFixed(2)} MiB`,
}));
