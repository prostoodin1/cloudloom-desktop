use super::Appearance;
use std::{ffi::c_void, mem::size_of};
use tokio::process::Child;
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::GetLogicalDrives,
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify},
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_CREATED_NEW_KEY},
};

// The handle stays in the parent only: Windows kills this mount process on a parent crash.
pub struct Job(usize);
impl Job {
    pub fn attach(child: &Child) -> Result<Self, String> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err("Не удалось создать защиту процесса диска".into());
            }
            let job = Self(handle as usize);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
                || AssignProcessToJobObject(
                    handle,
                    child.raw_handle().ok_or("Процесс диска недоступен")?,
                ) == 0
            {
                return Err("Не удалось защитить процесс диска; подключение отменено".into());
            }
            Ok(job)
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0 as *mut c_void);
        }
    }
}
pub fn letters() -> Vec<String> {
    let used = unsafe { GetLogicalDrives() };
    if used == 0 {
        return vec![];
    }
    (b'D'..=b'Z')
        .rev()
        .filter(|b| used & (1 << (b - b'A')) == 0)
        .map(|b| (b as char).to_string())
        .collect()
}
pub fn installed() -> bool {
    for key in [r"SOFTWARE\WOW6432Node\WinFsp", r"SOFTWARE\WinFsp"] {
        if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key) {
            if let Ok(dir) = key.get_value::<String, _>("InstallDir") {
                if std::path::Path::new(&dir)
                    .join("bin/winfsp-x64.dll")
                    .is_file()
                {
                    return true;
                }
            }
        }
    }
    false
}
fn key_path(letter: &str) -> Result<String, String> {
    if letter.len() != 1 || !(b'D'..=b'Z').contains(&letter.as_bytes()[0]) {
        return Err("Неверная буква оформления".into());
    }
    Ok(format!(
        r"Software\Classes\Applications\Explorer.exe\Drives\{letter}"
    ))
}
fn notify(letter: &str, mounted: bool) {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST | windows_sys::Win32::UI::Shell::SHCNF_FLUSH,
            std::ptr::null(),
            std::ptr::null(),
        );
        let path: Vec<u16> = format!("{letter}:\\")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        SHChangeNotify(
            if mounted {
                windows_sys::Win32::UI::Shell::SHCNE_DRIVEADD
            } else {
                windows_sys::Win32::UI::Shell::SHCNE_DRIVEREMOVED
            } as i32,
            windows_sys::Win32::UI::Shell::SHCNF_PATHW | windows_sys::Win32::UI::Shell::SHCNF_FLUSH,
            path.as_ptr().cast(),
            std::ptr::null(),
        );
        SHChangeNotify(
            windows_sys::Win32::UI::Shell::SHCNE_UPDATEITEM as i32,
            windows_sys::Win32::UI::Shell::SHCNF_PATHW | windows_sys::Win32::UI::Shell::SHCNF_FLUSH,
            path.as_ptr().cast(),
            std::ptr::null(),
        );
    }
}
pub fn appearance(a: &Appearance, icon: bool) -> Result<(), String> {
    let current = RegKey::predef(HKEY_CURRENT_USER);
    let path = key_path(&a.letter)?;
    let (key, disposition) = current
        .create_subkey(&path)
        .map_err(|_| "Диск подключён, но Windows не разрешила оформить его")?;
    if disposition != REG_CREATED_NEW_KEY {
        return Err("Диск подключён; существующее оформление этой буквы сохранено. Значок/подпись Cloudloom не применены".into());
    }
    key.set_value("CloudloomOwner", &a.owner)
        .map_err(|_| "Не удалось сохранить владельца оформления")?;
    if icon {
        key.create_subkey("DefaultIcon")
            .and_then(|(k, _)| k.set_value("", &a.icon))
            .map_err(|_| "Диск подключён, но значок не применён")?;
    }
    key.create_subkey("DefaultLabel")
        .and_then(|(k, _)| k.set_value("", &a.label))
        .map_err(|_| "Диск подключён, но название Проводника не применено")?;
    notify(&a.letter, true);
    Ok(())
}
pub fn cleanup(a: &Appearance) -> Result<(), String> {
    let current = RegKey::predef(HKEY_CURRENT_USER);
    let path = key_path(&a.letter)?;
    let key = match current.open_subkey_with_flags(&path, KEY_READ | KEY_WRITE) {
        Ok(k) => k,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("Не удалось восстановить оформление диска".into()),
    };
    if key.get_value::<String, _>("CloudloomOwner").ok().as_deref() != Some(&a.owner) {
        return Ok(());
    }
    for (name, expected) in [("DefaultIcon", &a.icon), ("DefaultLabel", &a.label)] {
        if let Ok(child) = key.open_subkey_with_flags(name, KEY_READ | KEY_WRITE) {
            if child.get_value::<String, _>("").ok().as_ref() == Some(expected) {
                child
                    .delete_value("")
                    .map_err(|_| "Не удалось убрать оформление Cloudloom")?;
            }
            drop(child);
            // Non-recursive deletion fails safely if the user added other values/subkeys.
            let _ = key.delete_subkey(name);
        }
    }
    key.delete_value("CloudloomOwner")
        .map_err(|_| "Не удалось убрать метку оформления")?;
    drop(key);
    let _ = current.delete_subkey(path);
    notify(&a.letter, false);
    Ok(())
}
