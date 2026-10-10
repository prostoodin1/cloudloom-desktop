//! Revocable loopback-only byte-range proxy. Credentials never enter the WebView.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::Response,
    routing::get,
};
use futures_util::TryStreamExt;
use std::{io, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpListener,
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tokio_util::{io::StreamReader, sync::CancellationToken};

pub(crate) enum Source {
    #[cfg(test)]
    Fixture {
        ranges: Arc<std::sync::Mutex<Vec<(u64, u64)>>>,
    },
    Delivery {
        client: Arc<crate::delivery::Client>,
        path: String,
    },
    Rclone {
        binary: PathBuf,
        config: PathBuf,
        password: String,
        remote: String,
    },
}
type Reader = Box<dyn AsyncRead + Send + Unpin>;
// A dropped HTTP body drops the child, cancelling its network transfer.
struct Input {
    reader: Reader,
    child: Option<tokio::process::Child>,
    left: u64,
    cancel: CancellationToken,
    _permit: OwnedSemaphorePermit,
}
impl Input {
    async fn chunk(&mut self) -> io::Result<Option<Bytes>> {
        if self.left == 0 {
            return Ok(None);
        }
        let mut bytes = vec![0; self.left.min(64 * 1024) as usize];
        let count = tokio::select! {
            _ = self.cancel.cancelled() => return Err(io::Error::other("Preview closed")),
            result = tokio::time::timeout(Duration::from_secs(90), self.reader.read(&mut bytes)) => result.map_err(|_| io::Error::other("Stream timeout"))??,
        };
        if count == 0 {
            return Err(io::Error::other("Remote stream ended early"));
        }
        self.left -= count as u64;
        bytes.truncate(count);
        // Keep ownership of the process until the body is consumed or cancelled.
        let _ = &self.child;
        Ok(Some(Bytes::from(bytes)))
    }
}
impl Source {
    async fn open(
        &self,
        start: u64,
        end: u64,
        size: u64,
        cancel: CancellationToken,
        permit: OwnedSemaphorePermit,
    ) -> Result<Input, String> {
        let count = end - start + 1;
        let (reader, child): (Reader, _) = match self {
            #[cfg(test)]
            Self::Fixture { ranges } => {
                ranges.lock().unwrap().push((start, end));
                (Box::new(tokio::io::repeat(42)), None)
            }
            Self::Delivery { client, path } => {
                let response = client.range(path, start, end, size).await?;
                let stream = response
                    .bytes_stream()
                    .map_err(|_| io::Error::other("Remote stream interrupted"));
                (Box::new(StreamReader::new(stream)), None)
            }
            Self::Rclone {
                binary,
                config,
                password,
                remote,
            } => {
                let mut command = tokio::process::Command::new(binary);
                command
                    .args([
                        "--config",
                        &config.to_string_lossy(),
                        "--ask-password=false",
                        "cat",
                        remote,
                        "--offset",
                        &start.to_string(),
                        "--count",
                        &count.to_string(),
                        "--retries",
                        "1",
                        "--low-level-retries",
                        "1",
                        "--buffer-size",
                        "64Ki",
                    ])
                    .env("RCLONE_CONFIG_PASS", password)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .kill_on_drop(true);
                #[cfg(windows)]
                command.creation_flags(0x08000000);
                let mut child = command.spawn().map_err(|_| "Не удалось открыть поток")?;
                let stdout = child.stdout.take().ok_or("Нет потока")?;
                (Box::new(stdout), Some(child))
            }
        };
        Ok(Input {
            reader,
            child,
            left: count,
            cancel,
            _permit: permit,
        })
    }
}

pub(crate) struct Session {
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}
struct Media {
    source: Source,
    size: u64,
    mime: &'static str,
    host: String,
    cancel: CancellationToken,
    slots: Arc<Semaphore>,
}

fn mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        _ => "video/mp4",
    }
}
// Single HTTP byte range, inclusive; all arithmetic stays 64-bit, including 50 GB.
fn range(header: &str, size: u64) -> Result<(u64, u64), ()> {
    let value = header.strip_prefix("bytes=").ok_or(())?;
    let (start, end) = value.split_once('-').ok_or(())?;
    if !start.bytes().all(|c| c.is_ascii_digit()) || !end.bytes().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    if size == 0 {
        return Err(());
    }
    if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        return Ok((size.saturating_sub(suffix), size - 1));
    }
    let start: u64 = start.parse().map_err(|_| ())?;
    let end: u64 = if end.is_empty() {
        size - 1
    } else {
        end.parse().map_err(|_| ())?
    };
    if start >= size || end < start {
        return Err(());
    }
    Ok((start, end.min(size - 1)))
}
fn allowed_origin(origin: &str) -> bool {
    matches!(
        origin,
        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
    ) || (cfg!(debug_assertions)
        && matches!(origin, "http://localhost:1420" | "http://127.0.0.1:1420"))
}
async fn serve(State(media): State<Arc<Media>>, method: Method, headers: HeaderMap) -> Response {
    if media.cancel.is_cancelled() {
        return Response::builder().status(410).body(Body::empty()).unwrap();
    }
    if headers.get("host").and_then(|v| v.to_str().ok()) != Some(media.host.as_str()) {
        return Response::builder().status(403).body(Body::empty()).unwrap();
    }
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    if origin.is_some_and(|v| !allowed_origin(v)) {
        return Response::builder().status(403).body(Body::empty()).unwrap();
    }
    let mut response = Response::builder()
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if let Some(origin) = origin {
        response = response
            .header("Access-Control-Allow-Origin", origin)
            .header("Vary", "Origin");
    }
    if method == Method::OPTIONS {
        return response
            .status(204)
            .header("Access-Control-Allow-Methods", "GET, HEAD")
            .header("Access-Control-Allow-Headers", "Range")
            .header("Access-Control-Allow-Private-Network", "true")
            .body(Body::empty())
            .unwrap();
    }
    let requested = if method == Method::HEAD {
        None
    } else {
        headers.get("range")
    };
    let (start, end) = match requested {
        Some(value) => match value.to_str().ok().and_then(|v| range(v, media.size).ok()) {
            Some(pair) => pair,
            None => {
                return response
                    .status(416)
                    .header("Content-Range", format!("bytes */{}", media.size))
                    .body(Body::empty())
                    .unwrap();
            }
        },
        None => (0, media.size.saturating_sub(1)),
    };
    response = response
        .header("Accept-Ranges", "bytes")
        .header("Content-Type", media.mime);
    if requested.is_some() {
        response = response.status(StatusCode::PARTIAL_CONTENT).header(
            "Content-Range",
            format!("bytes {start}-{end}/{}", media.size),
        );
    }
    let count = if media.size == 0 { 0 } else { end - start + 1 };
    if method == Method::HEAD || count == 0 {
        return response
            .header("Content-Length", count.to_string())
            .body(Body::empty())
            .unwrap();
    }
    let permit = match media.slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return Response::builder()
                .status(429)
                .header("Retry-After", "1")
                .body(Body::empty())
                .unwrap();
        }
    };
    let work = async {
        let mut input = media
            .source
            .open(start, end, media.size, media.cancel.clone(), permit)
            .await?;
        let first = input
            .chunk()
            .await
            .map_err(|_| "Поток недоступен")?
            .ok_or("Пустой поток")?;
        Ok::<_, String>((first, input))
    };
    let (first, input) = match tokio::select! { _ = media.cancel.cancelled() => return Response::builder().status(410).body(Body::empty()).unwrap(), result = tokio::time::timeout(Duration::from_secs(90), work) => result }
    {
        Ok(Ok(value)) => value,
        _ => return Response::builder().status(502).body(Body::empty()).unwrap(),
    };
    let stream =
        futures_util::stream::try_unfold((Some(first), input), |(first, mut input)| async move {
            if let Some(first) = first {
                return Ok(Some((first, (None, input))));
            }
            Ok::<_, io::Error>(input.chunk().await?.map(|chunk| (chunk, (None, input))))
        });
    response
        .header("Content-Length", count.to_string())
        .body(Body::from_stream(stream))
        .unwrap()
}
pub(crate) async fn start(
    source: Source,
    size: u64,
    name: &str,
) -> Result<(String, Session), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Не удалось открыть локальный видеопоток")?;
    let host = listener
        .local_addr()
        .map_err(|_| "Нет локального адреса")?
        .to_string();
    let secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let route = format!("/media/{secret}");
    let url = format!("http://{host}{route}");
    let cancel = CancellationToken::new();
    let media = Arc::new(Media {
        source,
        size,
        mime: mime(name),
        host,
        cancel: cancel.clone(),
        slots: Arc::new(Semaphore::new(4)),
    });
    let app = Router::new()
        .route(&route, get(serve).head(serve).options(serve))
        .with_state(media);
    let expiry = cancel.clone();
    let task = tokio::spawn(async move {
        tokio::select! {
            _ = axum::serve(listener, app) => {},
            _ = expiry.cancelled() => {},
            _ = tokio::time::sleep(Duration::from_secs(24 * 3600)) => { expiry.cancel(); },
        }
    });
    Ok((url, Session { cancel, task }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fifty_gb_metadata_and_tail_do_not_download_the_file() {
        let size = 50_000_000_000u64;
        let ranges = Arc::new(std::sync::Mutex::new(vec![]));
        let (url, session) = start(
            Source::Fixture {
                ranges: ranges.clone(),
            },
            size,
            "large.mp4",
        )
        .await
        .unwrap();
        let http = reqwest::Client::new();
        let head = http.head(&url).send().await.unwrap();
        assert_eq!(head.status(), 200);
        assert_eq!(head.headers()["content-length"], size.to_string());
        assert!(
            ranges.lock().unwrap().is_empty(),
            "HEAD must not open the cloud file"
        );
        let response = http
            .get(&url)
            .header("Range", "bytes=49999999998-")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(
            response.headers()["content-range"],
            "bytes 49999999998-49999999999/50000000000"
        );
        assert_eq!(response.headers()["content-length"], "2");
        assert_eq!(response.bytes().await.unwrap().as_ref(), &[42, 42]);
        assert_eq!(*ranges.lock().unwrap(), vec![(size - 2, size - 1)]);
        let response = http
            .request(Method::OPTIONS, &url)
            .header("Origin", "tauri://localhost")
            .header("Access-Control-Request-Method", "GET")
            .header("Access-Control-Request-Headers", "Range")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 204);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "tauri://localhost"
        );
        let active = http
            .get(&url)
            .header("Range", "bytes=0-")
            .send()
            .await
            .unwrap();
        drop(session);
        assert!(
            tokio::time::timeout(Duration::from_secs(3), active.bytes())
                .await
                .unwrap()
                .is_err(),
            "Closing the session must cancel an active 50 GB stream"
        );
    }
    #[test]
    fn ranges_are_64_bit_and_strict() {
        let size = 50_000_000_000;
        assert_eq!(
            range("bytes=49999999000-", size),
            Ok((size - 1000, size - 1))
        );
        assert_eq!(range("bytes=-1000", size), Ok((size - 1000, size - 1)));
        assert_eq!(range("bytes=3-999", 10), Ok((3, 9)));
        for value in [
            "bytes=10-",
            "bytes=4-3",
            "bytes=0-1,4-5",
            "bytes=-0",
            "bytes=18446744073709551616-",
            "other=0-1",
        ] {
            assert!(range(value, 10).is_err(), "{value}");
        }
        assert!(range("bytes=0-", 0).is_err());
        assert!(!allowed_origin("https://evil.example"));
    }
}
