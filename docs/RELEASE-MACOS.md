# Cloudloom Bloom for Mac

Two native builds, requiring macOS 13 Ventura or newer:

- **Cloudloom_Bloom_macOS_Intel.dmg** — Intel / x86_64.
- **Cloudloom_Bloom_macOS_AppleSilicon.dmg** — M-series / ARM64, no Rosetta required.
- Application ZIPs and SHA256 checksums accompany both builds.

Includes the Bloom profile selector, light/dark themes, real file/folder operations, 18 storage adapters and individual credential-help panels. macOS stores the configuration encryption password in Keychain. Each app contains the correct native rclone engine.

**Important:** these builds are ad-hoc signed, **not Developer ID signed or Apple-notarized**. Publisher identity is not verified by Apple, and Gatekeeper may block downloaded copies. Apple Developer signing/notarization is not configured. Do not disable macOS security to install this beta.

CI verifies both CPU architectures, packaged resources, engine execution, application signature integrity, native tests and a startup smoke test. Interactive Mac use, provider account consent and download/Gatekeeper behaviour still require validation on the target computer.

Public source contains no private repository history, local account data or credentials. Existing application licensing is unchanged.
