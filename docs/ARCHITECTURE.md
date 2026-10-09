# Architecture

React renders the profile picker, file manager, transfer progress and local service-help catalogue. Native operations are available only inside Tauri; browser previews do not invent data.

Rust owns encrypted per-profile rclone configurations. A random encryption password is stored in the OS credential vault: macOS Keychain or Windows Credential Manager. Configuration files are encrypted before any credentials are written. Profile metadata contains only name, provider, root and readiness; tokens are not exposed to React.

The bundled rclone executable is architecture-specific, selected and checksum-verified at preparation time. macOS bundles `resources/rclone`; Windows bundles `resources/rclone.exe`. No external rclone installation or user shell PATH is needed at runtime.

Bounded native jobs stream actual JSON statistics, allow cancellation and mark interrupted work on restart. Uploads and downloads reject existing destinations. Cloud permissions and filesystem semantics remain provider-dependent.

The webview has no general shell execution permission. The native documentation command only opens fixed, allowlisted rclone documentation URLs with the operating system's URL handler.

Mac builds are currently ad-hoc signed, without Developer ID or notarization. The CI smoke test checks package contents, native architecture, engine execution, signature integrity and application startup. Account authorization and real desktop interaction are not tested by that smoke test.
