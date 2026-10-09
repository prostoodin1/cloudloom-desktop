# Cloudloom Bloom

Named releases replace numeric branding. Technical installer version: 0.3.0.

- Small **Помощь** button beside the selected service: dedicated credential and authorization guide for every service, without clearing your form.
- Clear distinction between browser OAuth, application passwords, API keys and Access Grants. Each guide links to the relevant rclone documentation in the system browser.
- Catalogue expanded from 10 to 18 connections: MEGA, Koofr, HiDrive, Backblaze B2, Azure Blob Storage, Storj, Seafile and SFTP added.
- Search in the service selector; the add-profile button focuses it.
- Native configuration wiring and credential masking for the new adapters; Seafile 2FA questions and SSH known_hosts verification.
- Existing profiles and encrypted credentials preserved. Light/dark themes retained.

Install **Cloudloom_Bloom_Setup.exe** (Windows x64), or extract the portable ZIP completely before running Cloudloom.exe. MSI is also included.

Verified locally: production build, lint, catalogue tests, native security tests, adapter fields checked against the bundled rclone engine, encrypted local-provider file/folder roundtrip and UI help/search checks. Actual cloud sign-in still requires the owner's credentials/consent and has not been live-tested for all providers. No credentials or accounts are fabricated.

Credential instructions are based on the [rclone provider documentation](https://rclone.org/overview/), checked while preparing this release. Secret values stay in encrypted local configurations; never send them in chat.
