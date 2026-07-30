export const releaseVersion = "0.12.0";
export const releaseTagUrl =
  "https://github.com/dzass3/codex-assistant/releases/tag/v0.12.0";

const assetBase =
  "https://github.com/dzass3/codex-assistant/releases/download/v0.12.0";

export const releaseAssets = [
  {
    architecture: "x64",
    format: "EXE",
    recommended: true,
    fileName: "Codex.Assistant_0.12.0_x64-setup.exe",
    bytes: 10_961_189,
    sha256: "57054E8A6549A1E7A2C1835AB340FF11A421AC0F6DDB2D5D3C5C3F3BF7B6A09E",
  },
  {
    architecture: "x64",
    format: "MSI",
    recommended: false,
    fileName: "Codex.Assistant_0.12.0_x64_en-US.msi",
    bytes: 15_163_392,
    sha256: "32D5CBE8FF83BBBD1918E3FA8AB0E1AFE2ED914792F773618C7B6ACD316BABBF",
  },
  {
    architecture: "ARM64",
    format: "EXE",
    recommended: true,
    fileName: "Codex.Assistant_0.12.0_arm64-setup.exe",
    bytes: 9_922_798,
    sha256: "C341D7936B54211887B9EB493914C6D5F125053DF2540B7F6EE857055BB93EC3",
  },
  {
    architecture: "ARM64",
    format: "MSI",
    recommended: false,
    fileName: "Codex.Assistant_0.12.0_arm64_en-US.msi",
    bytes: 15_011_840,
    sha256: "C24C9F873A1CCF021FC825D526ECFC81842AE02E06D6B5DFC8D1808532A3218F",
  },
].map((asset) => ({
  ...asset,
  url: `${assetBase}/${encodeURIComponent(asset.fileName)}`,
  sizeMiB: `${(asset.bytes / 1_048_576).toFixed(2)} MiB`,
}));
