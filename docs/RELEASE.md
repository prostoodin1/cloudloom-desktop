# Cloudloom Harbor — Windows and Mac

Regular public release, not a prerelease. Named version: **Harbor**. Install Harbor once to enable live presentation updates; older versions do not have this mechanism.

## New

- Used/total space and usage meters in the connected-profile sidebar and active profile header. Queries real account/disk quotas every minute. Services that do not expose a quota show unavailable/unknown, never fake zeroes. Quota is for the account/disk, not necessarily the selected folder; Google quota can include other Google services.
- Open images, video and audio inside Cloudloom. Video/audio include playback and seeking controls. Works through the common rclone file-reading path for all configured services with read access.
- UTF-8 text editor with **Save**, **Ctrl+S** on Windows and **Cmd+S** on Mac. Saves back to the original service. Closing with unsaved changes asks for confirmation.
- Checks whether remote text changed since opening and refuses to knowingly overwrite those changes. Same-size edits are uploaded; saved contents are verified before reporting success. Save failures leave edits in the editor.

## Downloads

- **Cloudloom_Harbor_Windows_x64_Setup.exe** — Windows 10/11 x64 installer.
- **Cloudloom_Harbor_Windows_x64_Portable.zip** — extract all files and run cloudloom.exe. Keep resources beside the executable; requires Microsoft Edge WebView2 Runtime.
- **Cloudloom_Harbor_macOS_Intel.dmg** — Intel Macs, macOS 13+.
- **Cloudloom_Harbor_macOS_AppleSilicon.dmg** — native M-series Macs, macOS 13+, no Rosetta.
- Mac app ZIPs and SHA256 checksums accompany the packages.

## Limits and verification

Preview downloads a temporary local copy before displaying it; this is not instant cloud streaming. Media limit: **1 GiB** per file. UTF-8 text limit: **2 MiB**; binary/other encodings are rejected, never silently converted. Active HTML/SVG/code is shown only as plain text. Office documents, PDFs and other unsupported formats can still be downloaded. Video/image/audio codecs depend on the operating system; unsupported codecs show a download fallback.

Only individual generated media files are exposed through Tauri's asset protocol, not the user's disk. Cache is capped at 3 GiB. Closing a preview removes its copy; abandoned files older than 24 hours are pruned when opening another preview. Local preview copies are not encrypted. Cloud downloads and saves may incur provider costs where applicable.

Conflict checking is content-based but not a provider-side atomic compare-and-swap. A simultaneous writer between the check and upload can still race; use one editor at a time. Provider permissions, quotas, native Google document export and provider-specific behaviour still apply.

Automated tests cover catalogue schemas/size bounds, rejected executable/authentication changes, provider setup fields against the bundled engine, HTTPS preset validation, quota unknown/zero values, plain-text handling, binary/size rejection, encrypted local-folder transfers, preview downloads, text save/readback, same-size edits and conflict refusal. Platform CI builds Windows x64 and native Intel/Apple Silicon Mac packages, runs tests and checks packaged application startup. Windows CI additionally checks installation/uninstallation. These are not exhaustive live-account tests of all 27 service entries. Live UI checks contact raw.githubusercontent.com without storage credentials, cookies, profile/file names or referrers; accepted catalogues are cached locally.

**Publisher signing is not configured.** Windows is not Authenticode signed (SmartScreen may warn/block). Mac is ad-hoc signed, not Developer ID signed or Apple-notarized (Gatekeeper may block). Do not disable system security. Trusted signing needs publisher-owned certificates/Apple Developer credentials.

Public source/downloads contain no private repository history, local profiles or credentials. Existing licensing is unchanged.
- **27 service entries**, up from 18: Delivery Studio Red, Nextcloud, ownCloud, Proton Drive, OpenDrive, Jottacloud, PikPak, Wasabi and Cloudflare R2 join the existing catalogue. Each has a dedicated help button, credential instructions and matching setup fields. Some entries are WebDAV/S3 presets, not separate protocols. Proton Drive's rclone adapter is beta.
- **Delivery** is a separate WebDAV/SFTPGo preset. Obtain its enabled HTTPS WebDAV endpoint and credentials from the server administrator. `https://delivery.studiored.es/web/` is the web login, not a confirmed WebDAV endpoint. Live account connectivity is not claimed or fabricated.
- **Live presentation updates without restarting or reloading the window**: welcome text, accent colours, service names/descriptions and individual instruction text are fetched from this repository's `live-ui/catalog.json`. Checks on startup, returning to the window and every minute while visible; manual check in the profile menu. Existing editor state and transfers are preserved. Validated data only; no downloaded JavaScript/HTML/CSS or credential/endpoint changes. Invalid data/offline mode keeps the bundled or last accepted catalogue. [How to publish changes](https://github.com/prostoodin1/cloudloom-desktop/blob/main/docs/LIVE-UI.md).
- Live data is not arbitrary hot-swapping of application code: new adapters, new forms/actions and filesystem/backend fixes still require a new binary. GitHub/CDN/network delays mean delivery is not guaranteed immediately after a save.

## Included from Glimmer
