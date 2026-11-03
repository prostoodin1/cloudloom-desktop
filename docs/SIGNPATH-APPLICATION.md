# Prepared application for free Windows signing

Status: draft for owner approval; not submitted and not approved.

Project: Cloudloom

Repository: https://github.com/prostoodin1/cloudloom-desktop

Downloads: https://github.com/prostoodin1/cloudloom-desktop/releases

License: MIT. Bundled open-source components retain their own licenses, including rclone and WinFsp.

Owner / proposed release-signing approver: GitHub user `prostoodin1`.

Description: Cloudloom is an open-source desktop file manager for user-selected cloud accounts. It supports direct uploads/downloads, file previews and optional Windows Explorer drives through rclone and WinFsp. Credentials stay in the OS vault/encrypted configuration. It includes an uninstaller and does not disable operating-system protection.

Build system: public GitHub Actions workflow `.github/workflows/macos.yml`, tagged source builds for Windows x64, macOS Intel and macOS Apple Silicon. The Windows package is an NSIS installer, with a portable ZIP also provided. Untrusted PRs receive no signing secrets.

Requested service: free Windows Authenticode signing under the SignPath Foundation open-source program, subject to Foundation review and eligibility. No paid service or billing enrollment is requested.

Code signing policy: https://github.com/prostoodin1/cloudloom-desktop/blob/main/docs/CODE-SIGNING.md

Before submission: owner confirms the contact address, the proposed approver, two-factor authentication and the Foundation terms. After approval, configure the assigned organization/project/policy IDs and API token through the service and GitHub encrypted settings. Do not publish personal contact information or tokens in this draft.
