# Cloudloom Linden — Windows and Mac

## Что нового

- **Облака как диски в Проводнике Windows.** Кнопка «Диски в Проводнике» в главном меню и боковой панели: профиль, свободная буква D–Z, своё название и значок сервиса. Название локальное, облачный аккаунт не переименовывается. Для смены настроек отключите диск и подключите снова. Windows может кэшировать подпись до обновления окна Проводника.
- **Режим только чтения по умолчанию.** Запись включается явно; копирование, папки, переименование и удаление действуют на облако в рамках прав аккаунта. Отдельного подтверждения Cloudloom для каждого удаления через Проводник нет.
- **Подключение при запуске Cloudloom** по желанию. Это не служба Windows и не автозапуск приложения при входе в ОС. Диск работает, пока запущено приложение; обычное закрытие окна блокируется до отключения дисков. Ручное отключение выключает автоподключение.
- **Кэш и контроль отправки.** Чтение по частям, запись через постоянный кэш; очередь, ошибки и dirty-метаданные открытых файлов блокируют отключение. Аварийное завершение родителя останавливает процесс диска через Windows Job Object; оставшиеся изменения сохраняются для повторного подключения того же профиля с записью. Это не атомарная блокировка внешних программ: закройте файлы и не начинайте новую запись во время отключения. Кэш не защищает от сбоя диска/питания и не является полной офлайн-копией.

Для дисков нужен **WinFsp**, устанавливаемый отдельно с официального сайта; кнопка помощи есть в приложении. Драйвер не устанавливается скрыто; могут понадобиться права администратора. Сам Cloudloom запускайте обычным пользователем, как Проводник.

Поддерживаются профили rclone: Google Drive, OneDrive, WebDAV, S3 и другие. **Delivery с входом через SFTPGo user API пока не поддерживается именно как диск.** Для отдельного WebDAV-профиля нужен настоящий адрес, полученный у администратора; адрес не угадывается. На Mac подключения в Finder пока нет.

Кэш **не зашифрован Cloudloom** и содержит локальные данные файлов. Целевой максимум — 10 ГиБ, резерв — 1 ГиБ; открытые и неотправленные файлы могут превышать мягкий лимит. Не удаляйте кэш с неотправленными изменениями и не редактируйте один файл одновременно несколькими клиентами. [Подробная инструкция](https://github.com/prostoodin1/cloudloom-desktop/blob/cloudloom-linden/docs/EXPLORER.md).

Локальный тест с настоящим WinFsp проверяет временный диск: чтение, запись, папки, переименование, удаление, режим чтения, значок через Windows Shell, смену имени, блокировку отключения с dirty-записью, авторизацию локального API, остановку процесса и автоподключение после перезапуска. Рабочие облака студии не используются; это не сертификация каждого сервиса и сторонней программы.

## Сохранённые возможности и исправления

- **Исправлен обрыв долгих загрузок Delivery.** Прежний 90-секундный тайм-аут ожидания HTTP-ответа работал и во время отправки файла. Теперь отправка может продолжаться, пока идёт прогресс; повтор запускается после 120 секунд отсутствия прогресса/ответа. Проверено тестами с задержанным ответом и зависшим соединением.
- **Надёжная загрузка 0.8.2.** Delivery повторяет временные сетевые сбои с паузой и обновляет истёкший токен. Файл отправляется под уникальным временным именем, проверяется побайтово и затем получает окончательное имя. При повторе папки совпадающие файлы проверяются побайтово и пропускаются; отличающиеся сохраняются с сообщением о конфликте. Проверка требует дополнительного чтения файла с сервера и сетевого трафика. После отмены или закрытия приложения могут остаться временные `.cloudloom-*.partial` файлы; они не считаются готовыми. У ошибочной/отменённой передачи доступна кнопка «Повторить» в текущем сеансе. Для обычных облаков увеличены повторы и тайм-ауты, защита от перезаписи действует и при повторе.
- **Автообновление при запуске.** До входа в рабочее пространство приложение проверяет новую версию, скачивает подписанный пакет, проверяет подпись и устанавливает обновление с перезапуском. Без сети или при ошибке можно продолжить в текущей версии. Повторный запуск открывает существующее окно, а не второй установщик. Juniper и более старым версиям нужен один ручной переход на Kestrel.
- **… → Получить ссылку** для файлов и папок у подключений с поддержкой обмена. Создание требует отдельного подтверждения. Для новых Delivery API-профилей — только чтение, пароль, срок 1–90 дней и отключение созданной ссылки. Права/политики сервера сохраняются. У других адаптеров срок, пароль и отзыв через приложение не обещаются.
- **Потоковые видео и аудио до 50 ГБ** без полной временной копии на диске. Перемотка использует чтение участков; возможности сервиса и системные кодеки влияют на результат. MKV/AVI/HEVC не гарантированно воспроизводятся на каждой системе. Просмотр не создаёт публичной ссылки. Трафик и тарифы провайдера продолжают действовать.
- Скриншоты меню и локальной папки сняты на нейтральных демонстрационных файлах. Рабочие облака, аккаунты, адреса и документы студии не показаны.
- Новые профили Delivery всегда запрашивают ваш адрес, логин и пароль. Никакой частный сервер не подставляется автоматически.

## Downloads

- **Cloudloom_Linden_Windows_x64_Setup.exe** — Windows 10/11 x64, рекомендован для автообновлений.
- **Cloudloom_Linden_Windows_x64_Portable.zip** — распаковать целиком; рядом с cloudloom.exe должна оставаться папка resources. Требует WebView2. Дальнейшее обновление выполняется установщиком, не portable-патчером.
- **Cloudloom_Linden_macOS_Intel.dmg** — Intel, macOS 13+.
- **Cloudloom_Linden_macOS_AppleSilicon.dmg** — M-чипы, macOS 13+.
- Mac ZIP, подписанные updater-пакеты, подписи, SHA256 и latest.json также доступны. На Mac скопируйте приложение в Applications, не запускайте для обновления внутри read-only DMG.

## Verification and limits

CI runs frontend/Rust tests and independently builds/verifies Windows x64, Mac Intel and Mac Apple Silicon. Resources, engine architecture and startup are checked; Windows installer/uninstaller tests use a disposable CI account. The release is published only when all platforms succeed. Updater packages are signed, version-bound and reject downgrades. See [startup update details](UPDATES.md).

Sharing and byte-range reads are tested with a disposable local SFTPGo account. Browser tests exercise dialogs, playback, seeking, text saves and themes; a real rclone/proxy playback test is separate from mocked UI commands. A virtual 50 GB source tests 64-bit ranges and cancellation, not a real 50 GB upload to a user's account. These checks do not certify every cloud account, codec, macOS WebKit playback path or a complete future-version OS upgrade on every system.

Images retain temporary previews up to 1 GiB; UTF-8 text editing is limited to 2 MiB, with Save / Ctrl+S / Cmd+S. Conflict detection is best-effort, not an atomic lock. Code, HTML and SVG are displayed as text, never executed. Failed/cancelled transfers can be resumed during the same app session. Cross-restart resumption is not implemented; after restarting, select the source again. New transfers still reject existing destinations, while the explicit retry is limited to its original source and target. Batch moves can leave partial results after failures/cancellation. Cross-profile moves and dragging out to Explorer/Finder are not implemented.

**Updater signatures are not publisher certificates.** Windows is not Authenticode signed; Mac is ad-hoc signed, not Developer ID signed or notarized. SmartScreen/Gatekeeper can warn or block. Do not disable OS security.

The organization publication is a clean snapshot without old commits/releases. The personal upstream retains its existing history; this release does not rewrite it. Neither publication includes local profiles, credentials, private signing keys or working studio data. Application licensing remains all rights reserved; third-party licenses remain unchanged.
