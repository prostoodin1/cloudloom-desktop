//! The immutable official MSI is bundled, never fetched or selected by the WebView.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, State};

#[derive(Default)]
pub(crate) struct Installer {
    pub busy: AtomicBool,
}
struct Guard<'a>(&'a AtomicBool);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub(crate) async fn install_winfsp(
    app: AppHandle,
    installer: State<'_, Installer>,
) -> Result<String, String> {
    if installer.busy.swap(true, Ordering::SeqCst) {
        return Err("Установка WinFsp уже запущена".into());
    }
    let _guard = Guard(&installer.busy);
    #[cfg(windows)]
    {
        use tauri::Manager;
        if crate::mounts::winfsp_installed() {
            return Ok("installed".into());
        }
        let path = if cfg!(debug_assertions) {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        } else {
            app.path()
                .resource_dir()
                .map_err(|_| "Папка приложения недоступна")?
        }
        .join("resources/winfsp/winfsp-2.1.25156.msi");
        tokio::task::spawn_blocking(move || windows::install(&path))
            .await
            .map_err(|_| "Проверка установщика прервана")?
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("WinFsp нужен только в Windows".into())
    }
}

#[cfg(windows)]
mod windows {
    use sha2::{Digest, Sha256};
    use std::{fs::OpenOptions, io::Read, os::windows::fs::OpenOptionsExt, path::Path};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0},
        Storage::FileSystem::FILE_SHARE_READ,
        System::{
            SystemInformation::GetSystemDirectoryW,
            Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        },
        UI::{
            Shell::{
                SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };
    const HASH: &str = "073a70e00f77423e34bed98b86e600def93393ba5822204fac57a29324db9f7a";
    fn verify(bytes: &[u8]) -> Result<(), String> {
        if format!("{:x}", Sha256::digest(bytes)) != HASH {
            return Err("Установщик WinFsp повреждён или заменён. Переустановите Cloudloom из официального релиза".into());
        }
        Ok(())
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn parameters(path: &Path) -> Result<Vec<u16>, String> {
        let text = path
            .to_str()
            .ok_or("Путь установщика содержит неподдерживаемые символы")?;
        let text = text.strip_prefix(r"\\?\").unwrap_or(text);
        let b = text.as_bytes();
        if b.len() < 4
            || !b[0].is_ascii_alphabetic()
            || b[1] != b':'
            || b[2] != b'\\'
            || text.contains('"')
            || text.chars().any(char::is_control)
            || text.encode_utf16().count() >= 260
        {
            return Err("Для установки WinFsp переместите Cloudloom в обычную короткую папку на локальном диске".into());
        }
        Ok(wide(&format!("/i \"{text}\" /norestart")))
    }
    fn result(code: u32) -> Result<String, String> {
        match code {
            0 => Ok("installed".into()),
            3010 | 1641 => Ok("restartRequired".into()),
            1602 => Err(
                "Установка WinFsp отменена. Можно продолжить работу без дисков Проводника".into(),
            ),
            1618 => Err("Windows уже устанавливает другую программу. Повторите позже".into()),
            _ => Err(format!(
                "Установка WinFsp завершилась с кодом {code}. Обычная работа Cloudloom доступна; повторите установку или откройте справку WinFsp"
            )),
        }
    }
    pub(super) fn install(path: &Path) -> Result<String, String> {
        let path = path
            .canonicalize()
            .map_err(|_| "В пакете Cloudloom отсутствует установщик WinFsp")?;
        // Deny writes/deletion while the validated MSI is being used by Windows Installer.
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .map_err(|_| "Не удалось открыть установщик WinFsp")?;
        if file.metadata().map_err(|_| "Установщик недоступен")?.len() != 2_191_360
        {
            return Err("Неверный размер установщика WinFsp".into());
        }
        let mut bytes = vec![];
        file.read_to_end(&mut bytes)
            .map_err(|_| "Не удалось проверить установщик")?;
        verify(&bytes)?;
        let mut directory = vec![0u16; 32768];
        let length =
            unsafe { GetSystemDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
        if length == 0 || length >= directory.len() {
            return Err("Не удалось найти Windows Installer".into());
        }
        let executable = wide(&format!(
            "{}\\msiexec.exe",
            String::from_utf16_lossy(&directory[..length])
        ));
        // No silent install and no reboot. The official MSI displays its own license/options.
        let parameters = parameters(&path)?;
        let verb = wide("runas");
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = executable.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            return Err(if unsafe { GetLastError() } == 1223 {
                "Windows: разрешение администратора отменено. WinFsp не установлен"
            } else {
                "Windows не смогла запустить официальный установщик WinFsp"
            }
            .into());
        }
        if info.hProcess.is_null() {
            return Err("Windows не вернула процесс установщика".into());
        }
        let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
        let mut code = 0;
        let status = unsafe { GetExitCodeProcess(info.hProcess, &mut code) };
        unsafe {
            CloseHandle(info.hProcess);
        }
        drop(file);
        if wait != WAIT_OBJECT_0 || status == 0 {
            return Err(
                "Не удалось определить результат установки. Проверьте WinFsp в Windows".into(),
            );
        }
        let reply = result(code)?;
        if reply == "installed" && !crate::mounts::winfsp_installed() {
            return Err(
                "WinFsp пока не обнаружен. Перезагрузите Windows и повторите подключение".into(),
            );
        }
        Ok(reply)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn package_and_result_validation_never_runs_an_installer() {
            let args = parameters(Path::new(r"\\?\C:\App folder\winfsp.msi")).unwrap();
            assert_eq!(
                String::from_utf16_lossy(&args[..args.len() - 1]),
                "/i \"C:\\App folder\\winfsp.msi\" /norestart"
            );
            assert!(parameters(Path::new(r"\\server\share\bad.msi")).is_err());
            assert!(parameters(Path::new("relative.msi")).is_err());
            let bytes = std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/winfsp/winfsp-2.1.25156.msi"),
            )
            .unwrap();
            verify(&bytes).unwrap();
            let mut corrupt = bytes;
            corrupt[10] ^= 1;
            assert!(verify(&corrupt).is_err());
            assert!(verify(b"not an installer").is_err());
            assert_eq!(result(0).unwrap(), "installed");
            assert_eq!(result(3010).unwrap(), "restartRequired");
            assert!(result(1602).unwrap_err().contains("отменена"));
            assert!(result(1618).is_err());
        }
    }
}
