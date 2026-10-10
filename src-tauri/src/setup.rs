//! Convenience URL normalization, without relaxing HTTPS or host verification.
pub(crate) fn cloud_url(provider: &str, input: &str, user: &str) -> Result<String, String> {
    let input = input.trim();
    let mut url = tauri::Url::parse(&if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    })
    .map_err(|_| "Введите адрес сайта хранилища")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Нужен HTTPS-адрес без логина и пароля в ссылке".into());
    }
    url.set_query(None);
    url.set_fragment(None);
    if url.path().contains("/remote.php/")
        || (provider == "owncloud" && url.path().contains("/dav/"))
    {
        return Ok(url.into());
    }
    let path = url.path().trim_end_matches('/');
    let end = path
        .find("/index.php")
        .or_else(|| path.find("/apps/"))
        .or_else(|| path.strip_suffix("/login").map(str::len));
    let prefix = end.map(|n| &path[..n]).unwrap_or(path).to_string();
    url.set_path(&format!("{prefix}/remote.php/"));
    if provider == "nextcloud" {
        if user.trim().is_empty() {
            return Err("Введите пользователя Nextcloud".into());
        }
        url.path_segments_mut()
            .map_err(|_| "Некорректный адрес")?
            .pop_if_empty()
            .extend(["dav", "files", user, ""]);
    } else {
        url.path_segments_mut()
            .map_err(|_| "Некорректный адрес")?
            .pop_if_empty()
            .extend(["webdav", ""]);
    }
    Ok(url.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_clouds_accept_site_or_existing_webdav_url() {
        assert_eq!(
            cloud_url(
                "nextcloud",
                "cloud.example.com/index.php/apps/files/?dir=/photos#x",
                "alice@example.com"
            )
            .unwrap(),
            "https://cloud.example.com/remote.php/dav/files/alice@example.com/"
        );
        assert_eq!(
            cloud_url("nextcloud", "https://cloud.example.com/team/", "a/b").unwrap(),
            "https://cloud.example.com/team/remote.php/dav/files/a%2Fb/"
        );
        assert_eq!(
            cloud_url("owncloud", "https://cloud.example.com", "user").unwrap(),
            "https://cloud.example.com/remote.php/webdav/"
        );
        let exact = "https://cloud.example.com/custom/remote.php/dav/files/user/";
        assert_eq!(cloud_url("nextcloud", exact, "other").unwrap(), exact);
        for url in ["http://example.com", "https://user:pass@example.com"] {
            assert!(cloud_url("nextcloud", url, "u").is_err());
        }
    }
}
