//! Private WebDAV adapter for the SFTPGo user API. Never exposes cloud credentials.
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    response::Response,
};
use futures_util::{StreamExt, TryStreamExt};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Context {
    client: Arc<crate::delivery::Client>,
    secret: String,
    origin: String,
    read_only: bool,
    temporary: PathBuf,
    slots: Arc<tokio::sync::Semaphore>,
    stop: CancellationToken,
}
pub(crate) struct Bridge {
    pub url: String,
    pub secret: String,
    stop: CancellationToken,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
impl Bridge {
    pub async fn start(
        client: Arc<crate::delivery::Client>,
        temporary: PathBuf,
        read_only: bool,
    ) -> Result<Self, String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "Не удалось открыть адаптер Delivery")?;
        let origin = format!(
            "http://{}",
            listener
                .local_addr()
                .map_err(|_| "Ошибка адреса адаптера")?
        );
        let secret = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        std::fs::create_dir_all(&temporary).map_err(|_| "Не удалось создать кэш Delivery")?;
        let stop = CancellationToken::new();
        let context = Context {
            client,
            secret: format!("Bearer {secret}"),
            origin: origin.clone(),
            read_only,
            temporary,
            slots: Arc::new(tokio::sync::Semaphore::new(8)),
            stop: stop.clone(),
        };
        let cancel = stop.clone();
        tokio::spawn(async move {
            let app = Router::new().fallback(handle).with_state(context);
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(cancel.cancelled_owned())
                .await;
        });
        Ok(Self {
            url: format!("{origin}/"),
            secret,
            stop,
        })
    }
}
fn path(uri: &str) -> Result<String, StatusCode> {
    let decoded = percent_encoding::percent_decode_str(uri)
        .decode_utf8()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let value = decoded.trim_matches('/');
    crate::engine::safe_path(value).map_err(|_| StatusCode::BAD_REQUEST)?;
    if value.chars().any(char::is_control)
        || value.split('/').any(|s| s.is_empty()) && !value.is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(value.into())
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn item(path: &str, value: &serde_json::Value) -> String {
    let mut url = tauri::Url::parse("http://localhost/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .clear()
        .extend(path.trim_start_matches('/').split('/'));
    let directory = value["IsDir"] == true;
    let href = format!(
        "{}{}",
        url.path(),
        if directory && !url.path().ends_with('/') {
            "/"
        } else {
            ""
        }
    );
    let modified = value["ModTime"]
        .as_str()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|v| {
            format!(
                "<d:getlastmodified>{}</d:getlastmodified>",
                v.with_timezone(&chrono::Utc)
                    .format("%a, %d %b %Y %H:%M:%S GMT")
            )
        })
        .unwrap_or_default();
    format!(
        "<d:response><d:href>{}</d:href><d:propstat><d:prop><d:displayname>{}</d:displayname><d:resourcetype>{}</d:resourcetype><d:getcontentlength>{}</d:getcontentlength></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
        xml(&href),
        xml(value["Name"].as_str().unwrap_or("")),
        if directory { "<d:collection/>" } else { "" },
        value["Size"].as_u64().unwrap_or(0)
    ).replace("</d:prop>", &format!("{modified}</d:prop>"))
}
fn response(status: StatusCode, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .body(body.into())
        .unwrap()
}
async fn handle(State(ctx): State<Context>, request: Request) -> Response {
    if request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|s| s.to_str().ok())
        != Some(ctx.secret.as_str())
    {
        return response(StatusCode::UNAUTHORIZED, Body::empty());
    }
    // No browser origin is permitted, including an authenticated cross-origin request.
    if request.headers().contains_key(header::ORIGIN) {
        return response(StatusCode::FORBIDDEN, Body::empty());
    }
    tokio::select! {
        _ = ctx.stop.cancelled() => response(StatusCode::SERVICE_UNAVAILABLE, Body::empty()),
        result = async {
            let _permit = ctx.slots.acquire().await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            dispatch(&ctx, request).await
        } => match result {
            Ok(value) => value,
            Err(status) => response(status, Body::empty()),
        }
    }
}
async fn dispatch(ctx: &Context, request: Request) -> Result<Response, StatusCode> {
    let (parts, body) = request.into_parts();
    let method = parts.method.as_str();
    let path = path(parts.uri.path())?;
    if ctx.read_only && !["GET", "HEAD", "OPTIONS", "PROPFIND"].contains(&method) {
        return Err(StatusCode::FORBIDDEN);
    }
    match method {
        "OPTIONS" => Ok(Response::builder()
            .header("DAV", "1")
            .header(
                "Allow",
                "OPTIONS, PROPFIND, GET, HEAD, PUT, MKCOL, DELETE, MOVE",
            )
            .body(Body::empty())
            .unwrap()),
        "PROPFIND" => {
            let depth = parts
                .headers
                .get("depth")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("infinity");
            if !["0", "1"].contains(&depth) {
                return Err(StatusCode::FORBIDDEN);
            }
            let value = ctx.client.stat(&path).await.map_err(api_error)?;
            let mut output = format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?><d:multistatus xmlns:d=\"DAV:\">{}",
                item(&format!("/{path}"), &value)
            );
            if depth == "1" && value["IsDir"] == true {
                for row in ctx.client.list(&path).await.map_err(api_error)? {
                    let name = row["Name"].as_str().ok_or(StatusCode::BAD_GATEWAY)?;
                    output.push_str(&item(
                        &format!(
                            "/{}{}{}",
                            path,
                            if path.is_empty() { "" } else { "/" },
                            name
                        ),
                        &row,
                    ));
                }
            }
            output.push_str("</d:multistatus>");
            Ok(Response::builder()
                .status(207)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .body(Body::from(output))
                .unwrap())
        }
        "GET" | "HEAD" => {
            let value = ctx.client.stat(&path).await.map_err(api_error)?;
            if value["IsDir"] == true {
                return Err(StatusCode::METHOD_NOT_ALLOWED);
            }
            let size = value["Size"].as_u64().ok_or(StatusCode::BAD_GATEWAY)?;
            let range = parts
                .headers
                .get(header::RANGE)
                .and_then(|v| v.to_str().ok());
            let (start, end) = byte_range(range, size)?;
            let length = if size == 0 { 0 } else { end - start + 1 };
            let mut builder = Response::builder()
                .status(if range.is_some() { 206 } else { 200 })
                .header(header::CONTENT_LENGTH, length)
                .header(header::ACCEPT_RANGES, "bytes");
            if range.is_some() {
                builder =
                    builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
            }
            let data = if method == "HEAD" || size == 0 {
                Body::empty()
            } else {
                let upstream = ctx
                    .client
                    .range(&path, start, end, size)
                    .await
                    .map_err(api_error)?;
                Body::from_stream(
                    upstream
                        .bytes_stream()
                        .take_until(ctx.stop.clone().cancelled_owned()),
                )
            };
            Ok(builder.body(data).unwrap())
        }
        "MKCOL" => {
            ctx.client.mkdir(&path).await.map_err(api_error)?;
            Ok(response(StatusCode::CREATED, Body::empty()))
        }
        "DELETE" => {
            let value = ctx.client.stat(&path).await.map_err(api_error)?;
            ctx.client
                .delete(&path, value["IsDir"] == true)
                .await
                .map_err(api_error)?;
            Ok(response(StatusCode::NO_CONTENT, Body::empty()))
        }
        "MOVE" => {
            let destination = parts
                .headers
                .get("destination")
                .and_then(|v| v.to_str().ok())
                .ok_or(StatusCode::BAD_REQUEST)?;
            let url = tauri::Url::parse(destination).map_err(|_| StatusCode::BAD_REQUEST)?;
            if url.origin().ascii_serialization() != ctx.origin
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(StatusCode::FORBIDDEN);
            }
            let destination = self::path(url.path())?;
            // Do not delete an existing destination to emulate overwrite: a failed move must preserve it.
            ctx.client
                .rename(&path, &destination)
                .await
                .map_err(api_error)?;
            Ok(response(StatusCode::CREATED, Body::empty()))
        }
        "PUT" => {
            if path.is_empty() {
                return Err(StatusCode::FORBIDDEN);
            }
            let temporary = Temporary(
                ctx.temporary
                    .join(format!("{}.upload", uuid::Uuid::new_v4())),
            );
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary.0)
                .await
                .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
            let mut stream = body.into_data_stream();
            use tokio::io::AsyncWriteExt;
            let mut size = 0u64;
            while let Some(bytes) = stream
                .try_next()
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?
            {
                size = size
                    .checked_add(bytes.len() as u64)
                    .ok_or(StatusCode::PAYLOAD_TOO_LARGE)?;
                file.write_all(&bytes)
                    .await
                    .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
            }
            if let Some(length) = parts.headers.get(header::CONTENT_LENGTH) {
                if length.to_str().ok().and_then(|v| v.parse::<u64>().ok()) != Some(size) {
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
            file.flush()
                .await
                .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;
            drop(file);
            ctx.client
                .upload(&path, &temporary.0, true, Arc::new(AtomicU64::new(0)))
                .await
                .map_err(api_error)?;
            Ok(response(StatusCode::CREATED, Body::empty()))
        }
        _ => Err(StatusCode::METHOD_NOT_ALLOWED),
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn api_error(error: String) -> StatusCode {
    if error.contains("не найдены") {
        StatusCode::NOT_FOUND
    } else if error.contains("одноимённый") {
        StatusCode::PRECONDITION_FAILED
    } else if error.contains("запретил") {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::BAD_GATEWAY
    }
}
fn byte_range(value: Option<&str>, size: u64) -> Result<(u64, u64), StatusCode> {
    let Some(value) = value else {
        return Ok((0, size.saturating_sub(1)));
    };
    let (start, end) = value
        .strip_prefix("bytes=")
        .and_then(|v| v.split_once('-'))
        .ok_or(StatusCode::RANGE_NOT_SATISFIABLE)?;
    let (start, end) = if start.is_empty() {
        let count = end
            .parse::<u64>()
            .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
        if count == 0 {
            return Err(StatusCode::RANGE_NOT_SATISFIABLE);
        }
        (size.saturating_sub(count), size.saturating_sub(1))
    } else {
        (
            start
                .parse::<u64>()
                .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?,
            if end.is_empty() {
                size.saturating_sub(1)
            } else {
                end.parse::<u64>()
                    .map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?
                    .min(size.saturating_sub(1))
            },
        )
    };
    if size == 0 || start >= size || start > end {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_and_paths() {
        assert_eq!(byte_range(Some("bytes=4-"), 10).unwrap(), (4, 9));
        assert_eq!(byte_range(Some("bytes=-4"), 10).unwrap(), (6, 9));
        for value in ["bytes=0-1,3-4", "bytes=-0", "bytes=10-", "bytes=4-2"] {
            assert!(byte_range(Some(value), 10).is_err());
        }
        assert!(byte_range(Some("bytes=0-"), 0).is_err());
        for value in ["/%2e%2e/secret", "/a%5cb", "/a%00b", "/a//b", "/a%3ab"] {
            assert!(path(value).is_err());
        }
        assert_eq!(
            path("/hello%20world/%D1%84.txt").unwrap(),
            "hello world/ф.txt"
        );
        let value = serde_json::json!({"Name":"<&", "Size":4,"IsDir":false});
        assert!(item("/a&b", &value).contains("a&amp;b"));
    }
}
