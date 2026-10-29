use super::*;
use std::{
    collections::HashMap,
    io::{BufRead, Read, Write},
    net::TcpListener,
    sync::atomic::AtomicBool,
};

struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn fixture() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut files: HashMap<String, Option<Vec<u8>>> = HashMap::from([("/".into(), None)]);
        let mut shares = HashMap::<String, Value>::new();
        let mut retry_uploads = 0u32;
        let mut dropped_upload_response = false;
        while !stopped.load(Ordering::Relaxed) {
            let (stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream);
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                headers.push_str(&line);
            }
            let lower = headers.to_ascii_lowercase();
            let length: usize = lower
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let mut request = first.split_whitespace();
            let method = request.next().unwrap();
            let url =
                tauri::Url::parse(&format!("http://localhost{}", request.next().unwrap())).unwrap();
            let query: HashMap<String, String> = url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            let path = query.get("path").map(String::as_str).unwrap_or("/");
            let endpoint = url.path().strip_prefix("/api/v2/user/").unwrap_or("");
            let mut status = 200;
            let mut response = b"{}".to_vec();
            let mut extra = String::new();
            if endpoint == "token" {
                if lower.contains("authorization: basic ") {
                    response = br#"{"access_token":"fixture-token"}"#.to_vec();
                } else {
                    status = 401;
                }
            } else if !headers.contains("Bearer fixture-token") {
                status = 401;
            } else if path == "/redirect" {
                status = 302;
                extra = "Location: http://127.0.0.1:9/do-not-send-credentials\r\n".into();
            } else if path == "/slow" {
                std::thread::sleep(Duration::from_millis(180));
            } else {
                match (method, endpoint) {
                    ("GET", "dirs") => {
                        if !matches!(files.get(path), Some(None)) {
                            status = 404;
                        } else {
                            let prefix = format!("{}/", path.trim_end_matches('/'));
                            let rows: Vec<Value> = files.iter().filter_map(|(key,value)| {
                            let name = key.strip_prefix(&prefix)?; if name.is_empty() || name.contains('/') { return None; }
                            Some(json!({"name":name,"mode":if value.is_none(){2147483648u64}else{420},"size":value.as_ref().map(Vec::len).unwrap_or(0),"last_modified":"2026-10-10T12:00:00Z"}))
                        }).collect();
                            response = serde_json::to_vec(&rows).unwrap();
                        }
                    }
                    ("POST", "dirs") => {
                        if files.contains_key(path) {
                            status = 409;
                        } else {
                            files.insert(path.into(), None);
                            status = 201;
                        }
                    }
                    ("GET", "files") => {
                        if let Some(Some(bytes)) = files.get(path) {
                            response = bytes.clone();
                            if let Some(range) = lower
                                .lines()
                                .filter(|_| path != "/ignore-range.bin")
                                .find_map(|line| line.strip_prefix("range: bytes="))
                            {
                                let (start, end) = range.trim().split_once('-').unwrap();
                                let start: usize = start.parse().unwrap();
                                let end: usize = end.parse().unwrap();
                                if start <= end && end < bytes.len() {
                                    response = bytes[start..=end].to_vec();
                                    status = 206;
                                    extra = format!(
                                        "Content-Range: bytes {start}-{end}/{}\r\n",
                                        bytes.len()
                                    );
                                } else {
                                    status = 416;
                                }
                            }
                        } else {
                            status = 404;
                        }
                    }
                    ("POST", "files/upload") => {
                        if path == "/retry-once.txt" && retry_uploads == 0 {
                            retry_uploads += 1;
                            status = 503;
                        } else {
                            files.insert(path.into(), Some(body));
                            status = 201;
                            // Server saved bytes but the connection died before the ACK.
                            if path.contains(".cloudloom-") && !dropped_upload_response {
                                dropped_upload_response = true;
                                continue;
                            }
                        }
                    }
                    ("POST", "shares") => {
                        let value: Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(value["scope"], 1);
                        assert_eq!(value["paths"].as_array().unwrap().len(), 1);
                        assert!(value["expires_at"].as_u64().unwrap() > 0);
                        assert_eq!(value["password"], "fixture-share-password");
                        shares.insert("fixture-share".into(), value);
                        status = 201;
                        extra = "X-Object-ID: fixture-share\r\n".into();
                    }
                    ("DELETE", "shares/fixture-share") => {
                        if shares.remove("fixture-share").is_none() {
                            status = 404;
                        }
                    }
                    ("POST", "file-actions/move") => {
                        let target = query.get("target").unwrap();
                        if files.contains_key(target) {
                            status = 409;
                        } else if !files.contains_key(path) {
                            status = 404;
                        } else {
                            let prefix = format!("{path}/");
                            let keys: Vec<_> = files
                                .keys()
                                .filter(|key| *key == path || key.starts_with(&prefix))
                                .cloned()
                                .collect();
                            for key in keys {
                                let value = files.remove(&key).unwrap();
                                files.insert(format!("{target}{}", &key[path.len()..]), value);
                            }
                        }
                    }
                    ("DELETE", "files" | "dirs") => {
                        if !files.contains_key(path) {
                            status = 404;
                        } else {
                            let prefix = format!("{path}/");
                            files.retain(|key, _| key != path && !key.starts_with(&prefix));
                        }
                    }
                    _ => status = 404,
                }
            }
            let stream = reader.get_mut();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{extra}\r\n",
                response.len()
            );
            let _ = stream.write_all(&response);
        }
    });
    Server {
        url,
        stop,
        thread: Some(thread),
    }
}

#[tokio::test]
async fn upload_deadline_tracks_inactivity_instead_of_total_duration() {
    let server = fixture();
    let progress = Arc::new(AtomicU64::new(0));
    let advancing = progress.clone();
    let pump = tokio::spawn(async move {
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            advancing.fetch_add(1, Ordering::Relaxed);
        }
    });
    let request = Http::new()
        .post(format!("{}api/v2/user/files/upload?path=/slow", server.url))
        .bearer_auth("fixture-token");
    assert!(
        upload_with_idle_timeout(request, progress, Duration::from_millis(100))
            .await
            .unwrap()
            .status()
            .is_success()
    );
    pump.abort();
    let request = Http::new()
        .post(format!("{}api/v2/user/files/upload?path=/slow", server.url))
        .bearer_auth("fixture-token");
    assert!(
        upload_with_idle_timeout(
            request,
            Arc::new(AtomicU64::new(0)),
            Duration::from_millis(60)
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn explorer_delivery_bridge_real_rclone_roundtrip() {
    let server = fixture();
    let mut client = Client::new(Credentials {
        site: "https://fixture.invalid/".into(),
        user: "fixture".into(),
        password: "fixture-only".into(),
    })
    .unwrap();
    client.base = tauri::Url::parse(&server.url).unwrap();
    let client = Arc::new(client);
    let root = std::env::temp_dir().join(format!("cloudloom-dav-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let bridge = crate::delivery_dav::Bridge::start(client.clone(), root.join("uploads"), false)
        .await
        .unwrap();
    let http = Http::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    assert_eq!(
        http.get(&bridge.url).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.get(&bridge.url)
            .bearer_auth(&bridge.secret)
            .header("Origin", "https://untrusted.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
        "resources/rclone.exe"
    } else {
        "resources/rclone"
    });
    assert!(
        binary.is_file(),
        "Prepare the pinned engine before running the integration suite"
    );
    let config = root.join("empty.conf");
    std::fs::write(&config, "").unwrap();
    let run = |args: Vec<String>| {
        let mut command = tokio::process::Command::new(&binary);
        command
            .arg("--config")
            .arg(&config)
            .arg("--retries=1")
            .arg("--low-level-retries=1")
            .env("RCLONE_CONFIG_STORAGE_TYPE", "webdav")
            .env("RCLONE_CONFIG_STORAGE_URL", &bridge.url)
            .env("RCLONE_CONFIG_STORAGE_VENDOR", "other")
            .env("RCLONE_CONFIG_STORAGE_BEARER_TOKEN", &bridge.secret)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .args(args)
            .kill_on_drop(true);
        async move {
            let output = tokio::time::timeout(Duration::from_secs(60), command.output())
                .await
                .expect("rclone timed out")
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }
    };
    let source = root.join("source.txt");
    let bytes = b"0123456789 bridge data";
    std::fs::write(&source, bytes).unwrap();
    run(vec!["mkdir".into(), "storage:folder".into()]).await;
    let name = "folder/тест & 100%20.txt";
    run(vec![
        "copyto".into(),
        source.to_string_lossy().into_owned(),
        format!("storage:{name}"),
    ])
    .await;
    let listed: Value =
        serde_json::from_slice(&run(vec!["lsjson".into(), "storage:folder".into()]).await).unwrap();
    assert_eq!(listed[0]["Name"], "тест & 100%20.txt");
    assert!(
        listed[0]["ModTime"]
            .as_str()
            .unwrap()
            .starts_with("2026-10-10")
    );
    let url = {
        let mut url = tauri::Url::parse(&bridge.url).unwrap();
        url.path_segments_mut()
            .unwrap()
            .clear()
            .extend(name.split('/'));
        url
    };
    let partial = http
        .get(url.clone())
        .bearer_auth(&bridge.secret)
        .header("Range", "bytes=4-8")
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(&partial.bytes().await.unwrap()[..], &bytes[4..=8]);
    assert_eq!(
        run(vec!["cat".into(), format!("storage:{name}")]).await,
        bytes
    );
    run(vec![
        "moveto".into(),
        format!("storage:{name}"),
        "storage:folder/renamed.txt".into(),
    ])
    .await;
    std::fs::write(&source, b"replacement").unwrap();
    run(vec![
        "copyto".into(),
        source.to_string_lossy().into_owned(),
        "storage:folder/renamed.txt".into(),
        "--ignore-times".into(),
    ])
    .await;
    assert_eq!(
        run(vec!["cat".into(), "storage:folder/renamed.txt".into()]).await,
        b"replacement"
    );
    let readonly = crate::delivery_dav::Bridge::start(client.clone(), root.join("readonly"), true)
        .await
        .unwrap();
    for method in ["PUT", "MKCOL", "DELETE", "MOVE"] {
        assert_eq!(
            http.request(
                Method::from_bytes(method.as_bytes()).unwrap(),
                format!("{}folder/renamed.txt", readonly.url)
            )
            .bearer_auth(&readonly.secret)
            .body("must not write")
            .send()
            .await
            .unwrap()
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        http.request(Method::from_bytes(b"PROPFIND").unwrap(), &bridge.url)
            .bearer_auth(&bridge.secret)
            .header("Depth", "infinity")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let invalid_move = http
        .request(
            Method::from_bytes(b"MOVE").unwrap(),
            format!("{}folder/renamed.txt", bridge.url),
        )
        .bearer_auth(&bridge.secret)
        .header("Destination", "https://untrusted.invalid/stolen.txt")
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_move.status(), StatusCode::FORBIDDEN);
    run(vec![
        "deletefile".into(),
        "storage:folder/renamed.txt".into(),
    ])
    .await;
    run(vec!["rmdir".into(), "storage:folder".into()]).await;
    assert!(client.list("").await.unwrap().is_empty());
    assert_eq!(std::fs::read_dir(root.join("uploads")).unwrap().count(), 0);
    #[cfg(windows)]
    if std::env::var("CLOUDLOOM_TEST_MOUNTS").as_deref() == Ok("1") {
        crate::mounts::verify_delivery_mount(client.clone(), root.join("mount")).await;
    }
    let old_url = bridge.url.clone();
    drop(bridge);
    drop(readonly);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(http.get(old_url).send().await.is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn user_api_file_folder_preview_and_error_roundtrip() {
    let server = fixture();
    let real = std::env::var("CLOUDLOOM_TEST_SFTPGO_URL").ok();
    let mut client = Client::new(Credentials {
        site: "https://fixture.invalid/".into(),
        user: "cloudloom-test".into(),
        password: "fixture-pass-not-a-user-secret".into(),
    })
    .unwrap();
    // HTTP is allowed only in this private test constructor, for the loopback fixture; production new() rejects it.
    let url = real.as_deref().unwrap_or(&server.url);
    let test_url = tauri::Url::parse(url).unwrap();
    assert_eq!(test_url.host_str(), Some("127.0.0.1"));
    client.base = test_url;
    client.login(None).await.unwrap();
    assert!(client.list("").await.unwrap().is_empty());
    let folder =
        std::env::temp_dir().join(format!("cloudloom-delivery-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let source = folder.join("source.txt");
    std::fs::write(&source, "Привет Delivery\r\n").unwrap();
    let progress = Arc::new(AtomicU64::new(0));
    client
        .upload("note.txt", &source, false, progress.clone())
        .await
        .unwrap();
    assert!(progress.load(Ordering::Relaxed) > 0);
    assert!(
        client
            .upload("note.txt", &source, false, progress.clone())
            .await
            .is_err()
    );
    assert_eq!(
        client.stat("note.txt").await.unwrap()["Size"],
        std::fs::metadata(&source).unwrap().len()
    );
    progress.store(0, Ordering::Relaxed);
    *client.token.lock().await = Some("expired-token".into());
    client
        .upload_inner("/retry-once.txt", &source, progress.clone())
        .await
        .unwrap();
    assert_eq!(
        client.stat("retry-once.txt").await.unwrap()["Size"],
        std::fs::metadata(&source).unwrap().len()
    );
    let downloaded = folder.join("downloaded.txt");
    let bytes = std::fs::read(&source).unwrap();
    let partial = client
        .range("note.txt", 2, 8, bytes.len() as u64)
        .await
        .unwrap();
    assert_eq!(partial.bytes().await.unwrap().as_ref(), &bytes[2..=8]);
    assert!(
        client
            .range("note.txt", 2, 8, bytes.len() as u64 + 1)
            .await
            .is_err()
    );
    assert!(
        client
            .create_share("", "fixture-share-password", 7)
            .await
            .is_err()
    );
    assert!(
        client
            .create_share("note.txt", "fixture-share-password", 0)
            .await
            .is_err()
    );
    let shared = client
        .create_share("note.txt", "fixture-share-password", 7)
        .await
        .unwrap();
    assert!(
        shared
            .url
            .starts_with(&format!("{url}web/client/pubshares/"))
    );
    assert!(shared.expires_at.is_some());
    assert!(shared.can_revoke);
    client
        .revoke_share(shared.id.as_deref().unwrap())
        .await
        .unwrap();
    assert!(client.revoke_share("../wrong").await.is_err());
    client
        .download("note.txt", &downloaded, 1000, progress.clone())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(&downloaded).unwrap()
    );
    assert!(
        client
            .download("note.txt", &downloaded, 1000, progress.clone())
            .await
            .is_err()
    );
    assert!(
        client
            .download("note.txt", &folder.join("bounded.txt"), 3, progress.clone())
            .await
            .is_err()
    );
    let local_dir = folder.join("tree");
    std::fs::create_dir_all(local_dir.join("empty")).unwrap();
    std::fs::write(local_dir.join("file.txt"), b"nested").unwrap();
    client
        .transfer("tree", &local_dir, true, true, progress.clone(), false)
        .await
        .unwrap();
    std::fs::write(local_dir.join("after-interruption.txt"), b"continued").unwrap();
    progress.store(0, Ordering::Relaxed);
    client
        .transfer("tree", &local_dir, true, true, progress.clone(), true)
        .await
        .unwrap();
    let copy = folder.join("tree-copy");
    client
        .transfer("tree", &copy, false, true, progress.clone(), false)
        .await
        .unwrap();
    assert!(copy.join("empty").is_dir());
    assert_eq!(std::fs::read(copy.join("file.txt")).unwrap(), b"nested");
    assert_eq!(
        std::fs::read(copy.join("after-interruption.txt")).unwrap(),
        b"continued"
    );
    // A same-sized but different destination must not be silently accepted or overwritten.
    std::fs::write(local_dir.join("file.txt"), b"CHANG!").unwrap();
    assert!(
        client
            .transfer("tree", &local_dir, true, true, progress.clone(), true)
            .await
            .is_err()
    );
    std::fs::write(local_dir.join("file.txt"), b"nested").unwrap();
    std::fs::write(copy.join("file.txt"), b"WRONG!").unwrap();
    assert!(
        client
            .transfer("tree", &copy, false, true, progress.clone(), true)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(copy.join("file.txt")).unwrap(), b"WRONG!");
    std::fs::write(copy.join("file.txt"), b"nested").unwrap();
    progress.store(0, Ordering::Relaxed);
    client
        .transfer("tree", &copy, false, true, progress.clone(), true)
        .await
        .unwrap();
    assert!(progress.load(Ordering::Relaxed) > 0);
    client.rename("tree", "renamed").await.unwrap();
    assert!(client.stat("tree").await.is_err());
    assert!(
        client.stat("renamed/empty").await.unwrap()["IsDir"]
            .as_bool()
            .unwrap()
    );
    assert!(client.rename("renamed", "note.txt").await.is_err());
    assert!(client.delete("", true).await.is_err());
    // Exercise the actual shared engine/preview/text-save code; no real rclone binary may be called for API profiles.
    let client = Arc::new(client);
    let engine = crate::engine::Engine::test_delivery(client.clone(), folder.clone());
    assert!(
        engine
            .share_capabilities("api-test")
            .await
            .unwrap()
            .supported
    );
    let (stream_url, stream) = crate::streaming::start(
        engine
            .stream_source("api-test", "storage:note.txt")
            .unwrap(),
        bytes.len() as u64,
        "note.mp4",
    )
    .await
    .unwrap();
    let partial = Http::new()
        .get(&stream_url)
        .header("Range", "bytes=2-8")
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), 206);
    assert_eq!(partial.bytes().await.unwrap().as_ref(), &bytes[2..=8]);
    drop(stream);
    assert!(
        engine
            .execute("api-test", vec!["lsjson".into(), "storage:".into()])
            .await
            .unwrap()
            .contains("renamed")
    );
    let preview = folder.join("preview.txt");
    assert!(
        engine
            .preview_download("api-test", "storage:note.txt", &downloaded, 1000)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(&downloaded).unwrap()
    );
    engine
        .preview_download("api-test", "storage:note.txt", &preview, 1000)
        .await
        .unwrap();
    let mut session = crate::preview::Session {
        profile_id: "api-test".into(),
        remote: "storage:note.txt".into(),
        local: preview,
        original: Some(std::fs::read(&source).unwrap()),
        stream: None,
    };
    engine
        .save_text_session(&mut session, "Edited via shared editor".into())
        .await
        .unwrap();
    let verification = folder.join("verified.txt");
    client
        .download("note.txt", &verification, 1000, progress.clone())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(verification).unwrap(),
        "Edited via shared editor"
    );
    client
        .upload("note.txt", &source, true, progress.clone())
        .await
        .unwrap();
    assert!(
        engine
            .save_text_session(&mut session, "Do not overwrite somebody else's edit".into())
            .await
            .unwrap_err()
            .contains("изменился")
    );
    if real.is_none() {
        client
            .upload("ignore-range.bin", &source, false, progress.clone())
            .await
            .unwrap();
        assert!(
            client
                .range("ignore-range.bin", 2, 8, bytes.len() as u64)
                .await
                .unwrap_err()
                .contains("Полное скачивание не начато")
        );
        client.delete("ignore-range.bin", false).await.unwrap();
        *client.token.lock().await = Some("expired-token".into());
        client.list("").await.unwrap(); // one bounded token refresh, not a repeated password loop.
        assert!(
            client
                .list("redirect")
                .await
                .unwrap_err()
                .contains("перенаправляет")
        );
    }
    client.delete("renamed", true).await.unwrap();
    client.delete("note.txt", false).await.unwrap();
    client.delete("retry-once.txt", false).await.unwrap();
    assert!(client.list("").await.unwrap().is_empty());
    // The test exclusively created this UUID directory; no user files are removed.
    std::fs::remove_dir_all(folder).unwrap();
}
