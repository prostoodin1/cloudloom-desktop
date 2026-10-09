# Cloudloom Bloom

A desktop file manager for multiple cloud accounts, with a profile picker, light/dark themes, real file/folder transfers and per-service setup help.

## Download for Windows and Mac

See [Releases](https://github.com/prostoodin1/cloudloom-desktop/releases).

- **Cloudloom_Bloom_Windows_x64_Setup.exe** — Windows 10/11 x64 installer, including WebView2 runtime setup when needed.
- **Cloudloom_Bloom_Windows_x64_Portable.zip** — extract all files, keep the resources folder and run cloudloom.exe. Requires WebView2 Runtime.
- **Cloudloom_Bloom_macOS_Intel.dmg** — Intel Macs (x86_64).
- **Cloudloom_Bloom_macOS_AppleSilicon.dmg** — Apple Silicon (M-series, ARM64), native build, no Rosetta required.
- ZIP application bundles and SHA256 checksums are also provided.

Requires **macOS 13 Ventura or later**. Open the DMG and copy Cloudloom into Applications.

These initial Mac builds are **ad-hoc signed, not Apple Developer ID signed or notarized**. They do not have Apple-verified publisher identity. macOS Gatekeeper may block downloaded copies. No instructions to disable system security are included. Developer ID signing/notarization requires the publisher's Apple Developer credentials and is not configured in this release.

## Features

- Centered profile selector: saved profiles left, searchable services right, blurred background.
- Small **Помощь** button after selecting a service: dedicated credential instructions without clearing the form.
- Actual folder browsing, file/folder upload/download, rename, confirmed deletion, multi-select, list/grid views and drag/drop.
- Bounded concurrent transfers, actual progress and cancellation. Interrupted transfers are labelled on restart; cross-restart resumption is not implemented.
- Configurations encrypted individually. The encryption password is held in **macOS Keychain**, or Windows Credential Manager on Windows. Tokens are not stored in browser localStorage.
- 18 adapters: Google Drive, OneDrive, Dropbox, Google Cloud Storage, Box, pCloud, Yandex Disk, WebDAV/Nextcloud, S3-compatible storage, MEGA, Koofr, HiDrive, Backblaze B2, Azure Blob Storage, Storj, Seafile, SFTP and local folders.

Access depends on account permissions and provider-specific credentials. Google Drive may require a personal Desktop OAuth client. Each service's help explains setup and links to [rclone documentation](https://rclone.org/overview/). Do not send passwords, keys or tokens in issue reports.

Existing destinations are rejected to avoid overwriting. Folder/prefix behaviour and operation costs depend on the provider. Cloud sign-in and large-transfer behaviour must still be checked with target accounts; no demo accounts or files are fabricated.

## Build on macOS

Install Node.js 22+, Rust stable and Xcode Command Line Tools, then:

```sh
npm ci
npm run prepare:engine
npm run lint
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build
```

Preparation downloads pinned rclone v1.75.2 for the host CPU and verifies its official archive SHA256. The application bundles the matching executable and MIT license. Build natively on the requested CPU: do not package an Intel rclone binary inside an ARM app or vice versa.

The GitHub workflow builds and tests independently on Intel and Apple Silicon runners, verifies packaged architectures and resource execution, and checks that the application stays running at startup. Runner smoke tests are not a substitute for user-interactive tests, real account consent or a Gatekeeper test after downloading the release.

Windows source builds require MSVC C++ Build Tools and WebView2. The same commands build Windows natively; use `npm run tauri build -- --bundles nsis` for the installer. CI checks installation, resources, engine execution, application startup and uninstallation on a clean Windows account.

The public Bloom release includes Windows and both Mac variants and is not marked as a prerelease. Windows downloads are not Authenticode signed: SmartScreen may warn or block them. Release status does not remove operating-system security checks. See [release notes](docs/RELEASE.md) for verification scope and known limitations.

## Public repository

This repository contains only the application source, not the private ideas repository or its history, local profiles, OAuth credentials or test fixtures. Public visibility does not grant an open-source license: the application's existing all-rights-reserved licensing is unchanged. Third-party components retain their respective licenses.
