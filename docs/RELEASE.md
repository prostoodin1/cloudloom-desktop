# Cloudloom Juniper — Windows and Mac

Regular public release. Named version: **Juniper**. Install this native update for the new Delivery connection; live presentation updates cannot add a backend to an older binary.

## Что нового

- **Delivery: адрес сайта, логин и пароль.** Можно вставить `https://files.example.com/web/` или адрес своего SFTPGo. Токен получается автоматически через пользовательский API, без отдельного WebDAV URL. Нужны разрешённый HTTP/API-вход и права на файлы. 2FA-код вводится в дополнительных настройках; после истечения сессии понадобится новый код. Старые WebDAV-профили сохранены без изменений.
- Доступны папки, загрузка/скачивание файлов и папок, перемещение, переименование, удаление, предпросмотр медиа и сохранение UTF-8 текстов. Пароль хранится в системном хранилище Windows/macOS. HTTPS проверяется; перенаправления с учётными данными не выполняются. API не сообщает достоверную общую квоту: показываем неизвестный объём.
- **Nextcloud и классический ownCloud:** стандартный адрес подключения определяется из адреса сайта. Полный WebDAV URL также принимается. Для ownCloud Spaces и нестандартных серверов нужен точный endpoint; при 2FA/SSO может требоваться пароль приложения.
- Проверены формы и подсказки **27 сервисов**. Необязательные поля скрыты в дополнительных настройках, основной способ входа указан над формой. OAuth получает токены через браузер. S3/R2/Wasabi и другие ключевые подключения по-прежнему требуют реальные ключи. Для SFTP добавлен выбор файлов SSH-ключа и проверенного known_hosts; проверка сервера не отключается.

## Downloads

- **Cloudloom_Juniper_Windows_x64_Setup.exe** — Windows 10/11 x64 installer.
- **Cloudloom_Juniper_Windows_x64_Portable.zip** — extract all files, keep resources beside cloudloom.exe; requires WebView2 Runtime.
- **Cloudloom_Juniper_macOS_Intel.dmg** — Intel Macs, macOS 13+.
- **Cloudloom_Juniper_macOS_AppleSilicon.dmg** — native M-series Macs, macOS 13+.
- Mac application ZIPs and SHA256 checksum files are also included.

## Verification and limits

Automated Delivery tests exercise authentication, listing, file/folder upload/download (including empty folders), moves, duplicate rejection, bounded previews, text save/readback, external-edit conflict refusal and protected root deletion. A disposable local SFTPGo account is used separately from mock HTTP tests. These are not live tests of your Delivery credentials or every provider account. Server policies and actual permissions still apply.

Platform CI builds Windows x64 and native Intel/Apple Silicon Mac packages, runs frontend/Rust tests, verifies package resources and application startup. Windows additionally checks installation/uninstallation. Browser tests use mock native IPC for interactions, text/media previews, themes and live presentation updates.

Existing features remain: double-click/Enter to open, within-profile drag moves, multi-selection, light/dark themes, real quota display where supported, temporary image/video/audio previews and UTF-8 text editor with Save / Ctrl+S / Cmd+S. Media downloads before playback (1 GiB limit); text limit 2 MiB. Code/HTML/SVG are never executed. System codecs determine media support. Preview cache cap is 3 GiB; abandoned copies expire after 24 hours on subsequent preview use. Local preview copies are not encrypted.

Transfers can leave partial files after cancellation/failure; cross-restart resumption is not supported. Existing transfer destinations are rejected. Text conflict detection and destination checks are best-effort, not remote atomic locks; avoid simultaneous external writers. Moves stay within one profile, not between providers or out to Explorer/Finder. Batch moves are not atomic. Provider operations may consume quota or incur costs.

Live UI updates remain validated JSON data only, not downloaded executable code. The catalogue stays compatible with older releases; new API authentication instructions are bundled in Juniper. New logic requires a binary update.

**Publisher signing is not configured:** Windows is not Authenticode signed; Mac is ad-hoc signed, not Developer ID signed or notarized. SmartScreen/Gatekeeper may warn or block. Do not disable system security. Trusted publisher signing requires publisher-owned certificates.

No private repository history, real credentials or local profiles are published. Existing application licensing is unchanged.
