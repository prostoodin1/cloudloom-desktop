# Application for free Windows signing

Status: submitted to SignPath Foundation on 2026-11-03 with owner-approved contact details and consent. The site confirmed receipt. Awaiting review; not approved and no publisher certificate is configured. Marketing communications were left unchecked.

Project: Cloudloom

Repository: https://github.com/prostoodin1/cloudloom-desktop

Downloads: https://github.com/prostoodin1/cloudloom-desktop/releases

License: MIT. Bundled open-source components retain their own licenses, including rclone and WinFsp.

Owner / proposed release-signing approver: GitHub user `prostoodin1`.

Description: Cloudloom is an open-source desktop file manager for user-selected cloud accounts. It supports direct uploads/downloads, file previews and optional Windows Explorer drives through rclone and WinFsp. Credentials stay in the OS vault/encrypted configuration. It includes an uninstaller and does not disable operating-system protection.

Build system: public GitHub Actions workflow `.github/workflows/macos.yml`, tagged source builds for Windows x64, macOS Intel and macOS Apple Silicon. The Windows package is an NSIS installer, with a portable ZIP also provided. Untrusted PRs receive no signing secrets.

Requested service: free Windows Authenticode signing under the SignPath Foundation open-source program, subject to Foundation review and eligibility. No paid service or billing enrollment is requested.

Code signing policy: https://github.com/prostoodin1/cloudloom-desktop/blob/main/docs/CODE-SIGNING.md

The owner approved submission and the Foundation terms. Contact information remains private. Two-factor authentication must be enabled for repository and signing-service access before connecting the signing account. After approval, configure the assigned organization/project/policy IDs and API token through the service and GitHub encrypted settings. Do not publish personal contact information or tokens in this public document.
