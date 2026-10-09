# Cloudloom 0.2.0

Replaces the 0.1.0 demo workspace with real profiles and native file operations.

- Centered profile selector: saved accounts left, services right, blurred background and add-profile action.
- Persistent light/dark themes and list/grid preferences.
- Real storage contents, folder navigation, file/folder upload/download, multi-select, rename and confirmed deletion.
- Streaming statistics, bounded jobs, cancellation and interrupted status on restart.
- Official browser authorization, configuration questions and multiple accounts.
- Encrypted configurations with the encryption password in Windows Credential Manager.
- Bundled rclone v1.75.2, downloaded with official archive SHA256 verification.

EXE installer is recommended. MSI and portable ZIP are included. Extract the portable ZIP completely before running Cloudloom.exe.

Verified: TypeScript build, lint, frontend test, Rust security tests and real encrypted local-provider file/folder round trip. Cloud sign-in depends on account permissions and valid OAuth registration; configure your own Google Desktop OAuth client for reliable Google Drive sign-in because the shared client is retiring.
