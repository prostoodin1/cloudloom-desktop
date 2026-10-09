# Cloudloom Bloom — Windows and Mac

This is the regular public Bloom release, not a GitHub prerelease.

## Downloads

- **Cloudloom_Bloom_Windows_x64_Setup.exe** — Windows 10/11 x64 installer.
- **Cloudloom_Bloom_Windows_x64_Portable.zip** — unpack the complete ZIP into a folder and run `cloudloom.exe`. Keep the `resources` folder alongside it. Requires Microsoft Edge WebView2 Runtime; the installer is recommended because it handles runtime installation.
- **Cloudloom_Bloom_macOS_Intel.dmg** — Intel Macs, macOS 13 Ventura or later.
- **Cloudloom_Bloom_macOS_AppleSilicon.dmg** — native M-series Macs, macOS 13 Ventura or later, no Rosetta required.
- Mac application ZIPs and SHA256 checksums for all platforms are included.

The existing Mac packages are unchanged from the earlier Bloom Mac preview. Windows packages are now included in the same release.

## Included

Profile picker, light/dark themes, 18 storage adapters, per-service credential help, actual file/folder browsing, upload/download, rename, confirmed deletion and transfer cancellation. Each package includes its native rclone engine. Encryption passwords use Windows Credential Manager or macOS Keychain.

## Verification and known limitations

Automated frontend and native tests cover path validation, secret redaction, provider fields and encrypted local-folder file operations. Mac packages passed native CPU/resource/signature-integrity checks and a startup smoke test on both architectures. Windows CI verifies installation, bundled resources, engine execution, application startup and uninstallation. These checks do not establish real cloud-account authorization or exhaustive interactive testing.

**Publisher signing is not configured.** Windows files are not Authenticode signed, so SmartScreen may warn or block them. Mac apps are ad-hoc signed, not Developer ID signed or Apple-notarized; Gatekeeper may block downloaded copies. A regular GitHub release does not remove these operating-system restrictions. Do not disable system security. Trusted publisher signing requires publisher-owned certificates/Apple Developer credentials.

Cloud providers require appropriate credentials and account permissions. Cross-restart transfer resumption is not implemented. Public source and downloads contain no private repository history, local profiles or credentials. Existing licensing is unchanged.
