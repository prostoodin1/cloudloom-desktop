# Startup updates

Cloudloom (Kestrel and newer) checks the official GitHub release channel before mounting the profile/file workspace. A newer signed package is downloaded, its signature and signed version are verified, then it is installed and the app restarts. Up-to-date clients open the workspace normally. Offline/check/download/verification failures show a safe **Continue without updating** option; the existing application is not intentionally replaced with unverified bytes. Windows/macOS may ask for installation permissions. A single-instance guard prevents a second startup update while a Cloudloom window is already running.

Juniper and earlier do not contain the native updater. Install Kestrel once manually; subsequent published native versions can update at startup. Changing source files on GitHub alone does not update native clients: build and publish a newer signed release first. Live JSON presentation updates remain separate.

## Release process

1. Increase the internal SemVer in package.json/package-lock.json, Cargo.toml/Cargo.lock and tauri.conf.json. Use a human-friendly word for the window/release/asset names.
2. Keep the updater public key and application identifier stable. The private updater key must remain outside the repository and in the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`. Never print it, commit it, attach it to a release or put it in an issue. Keep a secure backup; losing it prevents updates to existing clients.
3. Run tests, commit/push and create a `cloudloom-*` tag on the personal upstream. CI builds Windows x64, macOS Intel and macOS Apple Silicon. Untrusted PR builds never receive the signing secret.
4. Each platform produces signed updater artifacts. The publisher requires all platforms, creates latest.json with their real signatures/URLs, uploads a draft release, then publishes it as latest only after the uploads finish. Do not use empty placeholder signatures or replace a published version with different binaries.
5. Verify the manifest, signatures, SHA256 files and all download URLs. Validate an upgrade on disposable installations before broad rollout. Never run installation/uninstallation verification scripts against a user's live installation.

The update channel is deliberately fixed to the personal upstream at `prostoodin1/cloudloom-desktop`. The organization repository is a clean public source/release mirror; it does not switch installed users to another trust channel. Mirror the exact upstream binaries/signatures/manifest after the upstream release completes. Organization builds do not independently publish a conflicting automatic-update release.

Updater signatures are not Windows Authenticode or Apple Developer ID/notarization. Those publisher certificates are not configured. OS security warnings can still occur; do not disable OS protections. For Windows, the installer is the supported automatic-update distribution. A portable copy receives an installer-based update, which may register the application; it is not a portable-only patcher. On macOS, copy the application out of the read-only DMG before running it. Installation errors preserve/restore the previous app where supported by the updater; OS/installer errors can still require manual recovery.

Network requests go to public GitHub release endpoints. Cloud passwords, tokens, file names and file contents are never sent to the update channel. Release checks have a 12-second timeout; downloads have bounded connection/read/overall timeouts. Signatures require a signed version matching the manifest; downgrades are disabled.
