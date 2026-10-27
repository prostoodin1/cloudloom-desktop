use super::*;
#[tokio::test]
async fn real_windows_mount_read_write_readonly_and_appearance() {
    if std::env::var("CLOUDLOOM_TEST_MOUNTS").as_deref() != Ok("1") {
        return;
    }
    if let Ok(path) = std::env::var("CLOUDLOOM_TEST_CLEANUP_DIR") {
        cleanup_fixture(Path::new(&path));
    }
    assert!(
        platform::installed(),
        "WinFsp is required for this opt-in integration test"
    );
    let root = std::env::temp_dir().join(format!("cloudloom-mount-test-{}", uuid::Uuid::new_v4()));
    let _fixture = Fixture(root.clone());
    let storage = root.join("storage");
    std::fs::create_dir_all(&storage).unwrap();
    std::fs::write(storage.join("hello.txt"), b"read from real mounted drive").unwrap();
    let engine = Engine::test_mount(root.clone());
    engine
        .execute(
            "mount-test",
            vec![
                "config".into(),
                "create".into(),
                "storage".into(),
                "alias".into(),
                "remote".into(),
                storage.to_string_lossy().into_owned(),
                "--non-interactive".into(),
            ],
        )
        .await
        .unwrap();
    let mounts = Mounts::default();
    let mut options = Options {
        id: "mount-test".into(),
        letter: platform::letters()
            .first()
            .expect("Need one free drive letter")
            .clone(),
        name: format!(
            "Cloudloom QA {}",
            &uuid::Uuid::new_v4().simple().to_string()[..6]
        ),
        read_only: false,
        auto_mount: true,
        service_icon: true,
    };
    {
        let mut inner = mounts.inner.lock().await;
        inner.records.push(Record {
            options: options.clone(),
            appearance: None,
        });
        inner.initialized = true;
        mounts.start(&engine, &mut inner, &options).await.unwrap();
        assert!(
            inner.sessions[&options.id].warning.is_none(),
            "{:?}",
            inner.sessions[&options.id].warning
        );
        let appearance = inner.records[0].appearance.as_ref().unwrap();
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(format!(
                r"Software\Classes\Applications\Explorer.exe\Drives\{}\DefaultLabel",
                options.letter
            ))
            .unwrap();
        assert_eq!(key.get_value::<String, _>("").unwrap(), appearance.label);
        let session = &inner.sessions[&options.id];
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("{}/core/pid", session.endpoint))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "RC must reject unauthenticated clients"
        );
    }
    assert_eq!(mounts.active.load(Ordering::SeqCst), 1);
    let drive = PathBuf::from(format!("{}:\\", options.letter));
    let shell_icon = shell_value(&drive, windows_sys::Win32::UI::Shell::SHGFI_ICONLOCATION);
    let shell_label = shell_value(&drive, windows_sys::Win32::UI::Shell::SHGFI_DISPLAYNAME);
    println!("Windows Shell icon: {shell_icon}; label: {shell_label}");
    assert!(shell_label.contains(&options.name));
    assert!(
        shell_icon.contains("explorer-icons"),
        "Shell must actually select the custom service icon, not merely store it in the registry"
    );
    assert_eq!(
        std::fs::read(drive.join("hello.txt")).unwrap(),
        b"read from real mounted drive"
    );
    std::fs::create_dir(drive.join("Папка")).unwrap();
    use std::io::Write;
    let mut open = std::fs::File::create(drive.join("Папка/upload.txt")).unwrap();
    open.write_all(b"uploaded through actual WinFsp drive")
        .unwrap();
    open.flush().unwrap();
    assert!(
        stop(&engine, &mounts, options.id.clone()).await.is_err(),
        "Open dirty writer must block disconnection"
    );
    assert!(mounts.has_profile(&options.id).await);
    drop(open);
    for _ in 0..120 {
        if storage.join("Папка/upload.txt").is_file()
            && mounts.inner.lock().await.sessions[&options.id]
                .pending(true)
                .await
                .unwrap()
                == 0
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(
        std::fs::read(storage.join("Папка/upload.txt")).unwrap(),
        b"uploaded through actual WinFsp drive"
    );
    std::fs::rename(
        drive.join("Папка/upload.txt"),
        drive.join("Папка/renamed.txt"),
    )
    .unwrap();
    std::fs::remove_file(drive.join("Папка/renamed.txt")).unwrap();
    stop(&engine, &mounts, options.id.clone()).await.unwrap();
    assert_eq!(mounts.active.load(Ordering::SeqCst), 0);
    assert!(!storage.join("Папка/renamed.txt").exists());
    assert!(!mounts.inner.lock().await.records[0].options.auto_mount);
    options.read_only = true;
    options.name = format!(
        "Переименован {}",
        &uuid::Uuid::new_v4().simple().to_string()[..6]
    );
    {
        let mut inner = mounts.inner.lock().await;
        inner.records[0].options = options.clone();
        mounts.start(&engine, &mut inner, &options).await.unwrap();
    }
    assert!(std::fs::write(drive.join("must-not-write.txt"), b"denied").is_err());
    assert_eq!(
        std::fs::read(drive.join("hello.txt")).unwrap(),
        b"read from real mounted drive"
    );
    // Simulate parent termination. The Job Object must remove the owned mount;
    // the next application startup restores the saved name, mode and icon.
    drop(mounts);
    for _ in 0..100 {
        if platform::letters().contains(&options.letter) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(platform::letters().contains(&options.letter));
    let mounts = Mounts::default();
    {
        let mut inner = mounts.inner.lock().await;
        restore(&engine, &mounts, &mut inner).await.unwrap();
        assert!(
            inner.sessions.contains_key(&options.id),
            "Auto-reconnect failed: {:?}",
            inner.errors
        );
        assert_eq!(inner.records[0].options.name, options.name);
        assert!(inner.records[0].options.read_only);
    }
    let restored_label = shell_value(&drive, windows_sys::Win32::UI::Shell::SHGFI_DISPLAYNAME);
    println!(
        "Restored Shell label: {restored_label}; expected: {}",
        options.name
    );
    let cold = tokio::process::Command::new("powershell.exe").creation_flags(0x08000000).args(["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $shell = New-Object -ComObject Shell.Application; $shell.NameSpace(17).ParseName($env:CLOUDLOOM_TEST_DRIVE).Name"]).env("CLOUDLOOM_TEST_DRIVE", format!("{}:", options.letter)).output().await.unwrap();
    let cold_label = String::from_utf8_lossy(&cold.stdout);
    println!("Fresh Shell label: {cold_label}");
    assert!(cold.status.success());
    // A real parent restart creates new Shell clients, unlike this test's warm SHGetFileInfo cache.
    assert!(cold_label.contains(&options.name));
    stop(&engine, &mounts, options.id.clone()).await.unwrap();
    let parent = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    assert!(
        parent
            .open_subkey(format!(
                r"Software\Classes\Applications\Explorer.exe\Drives\{}",
                options.letter
            ))
            .is_err()
    );
    assert!(platform::letters().contains(&options.letter));
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cloudloom-mount-test-")
    );
    std::fs::remove_dir_all(&root).unwrap();
}
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        cleanup_fixture(&self.0);
    }
}
fn cleanup_fixture(path: &Path) {
    if !path.is_dir() {
        return;
    }
    let Ok(root) = path.canonicalize() else {
        return;
    };
    let Ok(temp) = std::env::temp_dir().canonicalize() else {
        return;
    };
    if root.parent() != Some(temp.as_path()) {
        return;
    }
    let Some(name) = root.file_name().and_then(|s| s.to_str()) else {
        return;
    };
    let Some(id) = name.strip_prefix("cloudloom-mount-test-") else {
        return;
    };
    if uuid::Uuid::parse_str(id).is_err() {
        return;
    }
    let Ok(bytes) = std::fs::read(root.join("explorer-disks.json")) else {
        return;
    };
    let Ok(records) = serde_json::from_slice::<Vec<Record>>(&bytes) else {
        return;
    };
    for record in records {
        if let Some(a) = record.appearance {
            for _ in 0..50 {
                if platform::letters().contains(&a.letter) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !platform::letters().contains(&a.letter) {
                return;
            }
            if platform_cleanup(&a).is_err() {
                return;
            }
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
fn shell_value(path: &Path, flags: u32) -> String {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGetFileInfoW};
    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitialize(p: *mut std::ffi::c_void) -> i32;
        fn CoUninitialize();
    }
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let hr = CoInitialize(std::ptr::null_mut());
        assert!(hr >= 0);
        let mut info: SHFILEINFOW = std::mem::zeroed();
        let result = SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        CoUninitialize();
        assert_ne!(result, 0);
        let chars = info.szDisplayName;
        String::from_utf16_lossy(
            &chars[..chars.iter().position(|c| *c == 0).unwrap_or(chars.len())],
        )
    }
}
