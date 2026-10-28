# Third-party software

Cloudloom source is licensed under MIT; see [LICENSE](../LICENSE). Third-party components keep their original licenses. The application and installer must retain the notices shipped with their components.

## WinFsp (Windows only)

WinFsp - Windows File System Proxy, Copyright (C) Bill Zissimopoulos.

Repository and corresponding source: https://github.com/winfsp/winfsp/tree/v2.1

Cloudloom ships the **unmodified official** WinFsp 2.1.25156 MSI, released at https://github.com/winfsp/winfsp/releases/tag/v2.1. WinFsp is licensed under GPLv3 with the project's FLOSS exception. The complete upstream license and exception are included at `resources/winfsp/License.txt` in Windows packages. Cloudloom's MIT license does not relicense WinFsp. The copyright notice and repository link also appear in Cloudloom's Explorer-disk dialog.

The bundled MSI SHA256 is `073a70e00f77423e34bed98b86e600def93393ba5822204fac57a29324db9f7a`. Preparation checks SHA256 and Windows Authenticode. Runtime checks the pinned hash again and keeps the MSI open without write/delete sharing while Windows Installer uses it. No client-supplied installer paths or URLs are accepted.

Installing WinFsp is optional and starts only from an explicit confirmation in Cloudloom. The official Windows Installer interface and UAC prompt remain visible; installation is not silent and does not force a reboot. Existing WinFsp is not replaced automatically. Uninstalling Cloudloom must not uninstall this shared system driver.

## rclone

rclone v1.75.2 is distributed under MIT. Its license is included as `resources/RCLONE-LICENSE.txt`. Source: https://github.com/rclone/rclone/tree/v1.75.2.

## Application dependencies

Tauri, React, lucide-react and other Rust/JavaScript dependencies retain their package licenses. The lockfiles identify the exact versions used in each build. Microsoft Windows/WebView2 and macOS system components are separate platform software, not covered by Cloudloom's MIT license.
