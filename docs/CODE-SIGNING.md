# Code signing policy

Cloudloom source and release engineering are maintained at [prostoodin1/cloudloom-desktop](https://github.com/prostoodin1/cloudloom-desktop). The owner `prostoodin1` approves release tags and any future publisher-signing requests. The Studio-Red-Barcelona repository is a clean source and release mirror.

## Current status

Automatic-update packages are signed with the project's existing updater key. These signatures verify package integrity and the release version. They do not provide Windows Authenticode or Apple Developer ID trust.

The application for Windows publisher signing through the free SignPath Foundation program was submitted on 2026-11-03 and is awaiting review. Marketing communications were not selected and no paid subscription was requested. No Foundation approval, certificate, subscription or trusted publisher signature is claimed until it has actually been granted and the resulting binaries have been verified. [Foundation conditions](https://signpath.org/terms), [application](https://signpath.org/apply).

macOS packages have an ad-hoc signature for bundle integrity. They are not Developer ID signed or notarized. Apple requires an eligible Developer Program membership for Developer ID distribution. Cloudloom will not install a private root certificate, remove quarantine attributes or disable operating-system security to hide warnings. [Apple Developer ID](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/).

## Release controls

- Build only reviewed repository source and tagged releases through GitHub Actions. Pull-request builds receive no signing credentials.
- The owner reviews changes submitted by other contributors and approves requests for publisher signing. Two-factor authentication must be enabled for the repository and signing-service accounts before connecting a Foundation subscription.
- Sign only Cloudloom binaries produced from this repository. Bundled third-party software retains its upstream licenses and signatures; upstream binaries are not represented as Cloudloom-authored code.
- A future Windows signing pipeline must sign the application before packaging, then sign the installer, and only then generate the updater signature and checksums. Never change a published package in place.
- Verify final signatures, signed versions and digests before publication. The organization mirror receives identical approved binaries.
- Private signing keys and API tokens belong in the signing service or GitHub encrypted secrets. Never include them in source, application profiles, release assets, issues or screenshots.

## Privacy

Cloudloom connects to the storage servers selected by its users. Credentials remain in the operating-system vault or encrypted local engine configuration. Public release checks do not receive cloud passwords, file contents or file names. Private studio endpoints are entered locally and are not distributed as defaults.
