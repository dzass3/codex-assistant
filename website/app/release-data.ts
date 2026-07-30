export const releaseVersion = "0.11.9";
export const releaseTagUrl =
  "https://github.com/dzass3/codex-assistant/releases/tag/v0.11.9";

const assetBase =
  "https://github.com/dzass3/codex-assistant/releases/download/v0.11.9";

export const releaseAssets = [
  {
    architecture: "x64",
    format: "EXE",
    recommended: true,
    fileName: "Codex.Assistant_0.11.9_x64-setup.exe",
    bytes: 7_054_583,
    sha256: "B4C4627E8E8157064F5E7C3F85268310A86ACBFA12ACA9774EF49071D4627AFA",
  },
  {
    architecture: "x64",
    format: "MSI",
    recommended: false,
    fileName: "Codex.Assistant_0.11.9_x64_en-US.msi",
    bytes: 12_021_760,
    sha256: "31F2E4B86F29F91DC426248D07492C5119803E5436C3453C589C348B1FF30238",
  },
  {
    architecture: "ARM64",
    format: "EXE",
    recommended: true,
    fileName: "Codex.Assistant_0.11.9_arm64-setup.exe",
    bytes: 6_755_175,
    sha256: "0CA870ADF44BEA0224183AADC83E42B740C257D55D1D9458B56578B969A950A5",
  },
  {
    architecture: "ARM64",
    format: "MSI",
    recommended: false,
    fileName: "Codex.Assistant_0.11.9_arm64_en-US.msi",
    bytes: 11_870_208,
    sha256: "D011402C30341D80A66C93FF404844A3060F024F0CD506A8C80908C9F993D25A",
  },
].map((asset) => ({
  ...asset,
  url: `${assetBase}/${encodeURIComponent(asset.fileName)}`,
  sizeMiB: `${(asset.bytes / 1_048_576).toFixed(2)} MiB`,
}));
