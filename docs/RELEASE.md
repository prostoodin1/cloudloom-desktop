# Cloudloom Kestrel 0.8.3 — Windows and Mac

## Что нового

- **Исправлен обрыв долгих загрузок Delivery.** Прежний 90-секундный тайм-аут ожидания HTTP-ответа работал и во время отправки файла. Теперь отправка может продолжаться, пока идёт прогресс; повтор запускается после 120 секунд отсутствия прогресса/ответа. Проверено тестами с задержанным ответом и зависшим соединением.
- **Надёжная загрузка 0.8.2.** Delivery повторяет временные сетевые сбои с паузой и обновляет истёкший токен. Файл отправляется под уникальным временным именем, проверяется побайтово и затем получает окончательное имя. При повторе папки совпадающие файлы проверяются побайтово и пропускаются; отличающиеся сохраняются с сообщением о конфликте. Проверка требует дополнительного чтения файла с сервера и сетевого трафика. После отмены или закрытия приложения могут остаться временные `.cloudloom-*.partial` файлы; они не считаются готовыми. У ошибочной/отменённой передачи доступна кнопка «Повторить» в текущем сеансе. Для обычных облаков увеличены повторы и тайм-ауты, защита от перезаписи действует и при повторе.
- **Автообновление при запуске.** До входа в рабочее пространство приложение проверяет новую версию, скачивает подписанный пакет, проверяет подпись и устанавливает обновление с перезапуском. Без сети или при ошибке можно продолжить в текущей версии. Повторный запуск открывает существующее окно, а не второй установщик. Juniper и более старым версиям нужен один ручной переход на Kestrel.
- **… → Получить ссылку** для файлов и папок у подключений с поддержкой обмена. Создание требует отдельного подтверждения. Для новых Delivery API-профилей — только чтение, пароль, срок 1–90 дней и отключение созданной ссылки. Права/политики сервера сохраняются. У других адаптеров срок, пароль и отзыв через приложение не обещаются.
- **Потоковые видео и аудио до 50 ГБ** без полной временной копии на диске. Перемотка использует чтение участков; возможности сервиса и системные кодеки влияют на результат. MKV/AVI/HEVC не гарантированно воспроизводятся на каждой системе. Просмотр не создаёт публичной ссылки. Трафик и тарифы провайдера продолжают действовать.
- Скриншоты меню и локальной папки сняты на нейтральных демонстрационных файлах. Рабочие облака, аккаунты, адреса и документы студии не показаны.
- Новые профили Delivery всегда запрашивают ваш адрес, логин и пароль. Никакой частный сервер не подставляется автоматически.

## Downloads

- **Cloudloom_Kestrel_Windows_x64_Setup.exe** — Windows 10/11 x64, рекомендован для автообновлений.
- **Cloudloom_Kestrel_Windows_x64_Portable.zip** — распаковать целиком; рядом с cloudloom.exe должна оставаться папка resources. Требует WebView2. Дальнейшее обновление выполняется установщиком, не portable-патчером.
- **Cloudloom_Kestrel_macOS_Intel.dmg** — Intel, macOS 13+.
- **Cloudloom_Kestrel_macOS_AppleSilicon.dmg** — M-чипы, macOS 13+.
- Mac ZIP, подписанные updater-пакеты, подписи, SHA256 и latest.json также доступны. На Mac скопируйте приложение в Applications, не запускайте для обновления внутри read-only DMG.

## Verification and limits

CI runs frontend/Rust tests and independently builds/verifies Windows x64, Mac Intel and Mac Apple Silicon. Resources, engine architecture and startup are checked; Windows installer/uninstaller tests use a disposable CI account. The release is published only when all platforms succeed. Updater packages are signed, version-bound and reject downgrades. See [startup update details](UPDATES.md).

Sharing and byte-range reads are tested with a disposable local SFTPGo account. Browser tests exercise dialogs, playback, seeking, text saves and themes; a real rclone/proxy playback test is separate from mocked UI commands. A virtual 50 GB source tests 64-bit ranges and cancellation, not a real 50 GB upload to a user's account. These checks do not certify every cloud account, codec, macOS WebKit playback path or a complete future-version OS upgrade on every system.

Images retain temporary previews up to 1 GiB; UTF-8 text editing is limited to 2 MiB, with Save / Ctrl+S / Cmd+S. Conflict detection is best-effort, not an atomic lock. Code, HTML and SVG are displayed as text, never executed. Failed/cancelled transfers can be resumed during the same app session. Cross-restart resumption is not implemented; after restarting, select the source again. New transfers still reject existing destinations, while the explicit retry is limited to its original source and target. Batch moves can leave partial results after failures/cancellation. Cross-profile moves and dragging out to Explorer/Finder are not implemented.

**Updater signatures are not publisher certificates.** Windows is not Authenticode signed; Mac is ad-hoc signed, not Developer ID signed or notarized. SmartScreen/Gatekeeper can warn or block. Do not disable OS security.

The organization publication is a clean snapshot without old commits/releases. The personal upstream retains its existing history; this release does not rewrite it. Neither publication includes local profiles, credentials, private signing keys or working studio data. Application licensing remains all rights reserved; third-party licenses remain unchanged.
