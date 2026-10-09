# Cloudloom Glimmer — Windows and Mac

Regular public release, not a prerelease. Named version: **Glimmer**.

## New

- Used/total space and usage meters in the connected-profile sidebar and active profile header. Queries real account/disk quotas every minute. Services that do not expose a quota show unavailable/unknown, never fake zeroes. Quota is for the account/disk, not necessarily the selected folder; Google quota can include other Google services.
- Open images, video and audio inside Cloudloom. Video/audio include playback and seeking controls. Works through the common rclone file-reading path for all configured services with read access.
- UTF-8 text editor with **Save**, **Ctrl+S** on Windows and **Cmd+S** on Mac. Saves back to the original service. Closing with unsaved changes asks for confirmation.
- Checks whether remote text changed since opening and refuses to knowingly overwrite those changes. Same-size edits are uploaded; saved contents are verified before reporting success. Save failures leave edits in the editor.

## Downloads

- **Cloudloom_Glimmer_Windows_x64_Setup.exe** — Windows 10/11 x64 installer.
- **Cloudloom_Glimmer_Windows_x64_Portable.zip** — extract all files and run cloudloom.exe. Keep resources beside the executable; requires Microsoft Edge WebView2 Runtime.
- **Cloudloom_Glimmer_macOS_Intel.dmg** — Intel Macs, macOS 13+.
- **Cloudloom_Glimmer_macOS_AppleSilicon.dmg** — native M-series Macs, macOS 13+, no Rosetta.
- Mac app ZIPs and SHA256 checksums accompany the packages.

## Limits and verification

Preview downloads a temporary local copy before displaying it; this is not instant cloud streaming. Media limit: **1 GiB** per file. UTF-8 text limit: **2 MiB**; binary/other encodings are rejected, never silently converted. Active HTML/SVG/code is shown only as plain text. Office documents, PDFs and other unsupported formats can still be downloaded. Video/image/audio codecs depend on the operating system; unsupported codecs show a download fallback.

Only individual generated media files are exposed through Tauri's asset protocol, not the user's disk. Cache is capped at 3 GiB. Closing a preview removes its copy; abandoned files older than 24 hours are pruned when opening another preview. Local preview copies are not encrypted. Cloud downloads and saves may incur provider costs where applicable.

Conflict checking is content-based but not a provider-side atomic compare-and-swap. A simultaneous writer between the check and upload can still race; use one editor at a time. Provider permissions, quotas, native Google document export and provider-specific behaviour still apply.

Automated tests cover quota unknown/zero values, plain-text handling, binary/size rejection, encrypted local-folder transfers, real preview downloads, remote text save/readback, same-size edits and conflict refusal. Platform CI builds Windows x64 and native Intel/Apple Silicon Mac packages, runs tests and checks packaged application startup. Windows CI additionally checks installation/uninstallation. These are not exhaustive live-account tests of all 18 providers.

**Publisher signing is not configured.** Windows is not Authenticode signed (SmartScreen may warn/block). Mac is ad-hoc signed, not Developer ID signed or Apple-notarized (Gatekeeper may block). Do not disable system security. Trusted signing needs publisher-owned certificates/Apple Developer credentials.

Public source/downloads contain no private repository history, local profiles or credentials. Existing licensing is unchanged.
