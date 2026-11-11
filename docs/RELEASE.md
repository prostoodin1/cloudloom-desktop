# Cloudloom Rowan

В «Общие настройки → При закрытии окна» можно выбрать сворачивание в трей, безопасное отключение дисков перед выходом или оставить окно открытым, пока диски подключены. Через значок Cloudloom можно вернуть окно или выйти. Файлы с незавершёнными изменениями блокируют отключение дисков и выход.

Cloudloom выполняет одну передачу из приложения за раз. Каждый диск Проводника отправляет по одному файлу, а rclone в Windows работает с пониженным приоритетом. Пока окно скрыто, проверка квот приостанавливается. Это снижает конкуренцию за ресурсы, но причина зависания на конкретном компьютере не воспроизведена.

Значки статусов остаются в Cloudloom. Значки для Windows Explorer доступны отдельным пакетом. Cloudloom сохраняет возможность продолжить скачивание Delivery при подтверждении версии файла сервером и ведёт месячный счётчик скачиваний.

## Загрузки

- **Cloudloom_Rowan_Windows_x64_Setup.exe** — Windows 10/11 x64 с автообновлением.
- **Cloudloom_Rowan_Windows_x64_Portable.zip** — распакуйте целиком. Нужен WebView2.
- **Cloudloom_Rowan_Windows_x64_ExplorerOverlay.zip** — необязательное расширение для Проводника. Инструкции находятся в README.txt.
- **Cloudloom_Rowan_macOS_Intel.dmg** — Intel Mac, macOS 13+.
- **Cloudloom_Rowan_macOS_AppleSilicon.dmg** — Mac с M-чипом, macOS 13+.

Автор проекта — [prostoodin1](https://github.com/prostoodin1). [Все версии](https://github.com/prostoodin1/cloudloom-desktop/releases).

Пакеты автообновления подписаны. Для Windows подпись издателя Authenticode пока недоступна, а сборки Mac имеют подпись ad-hoc. [Политика подписи](https://github.com/prostoodin1/cloudloom-desktop/blob/main/docs/CODE-SIGNING.md).
