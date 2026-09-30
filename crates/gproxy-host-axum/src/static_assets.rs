use bytes::Bytes;
use http::request::Parts;
use http::{HeaderValue, Method, Response, StatusCode};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/web"]
#[exclude = ".gitkeep"]
struct WebAssets;

pub(crate) fn asset_path(parts: &Parts) -> Option<&str> {
    if !is_read(parts) {
        return None;
    }
    let path = parts.uri.path();
    if path == "/build-info.js" {
        Some("build-info.js")
    } else if path == "/" || path == "/portal" || portal_section(path) {
        Some("index.html")
    } else {
        path.strip_prefix('/').filter(|path| {
            path.starts_with("assets/")
                || matches!(
                    *path,
                    "favicon.ico" | "favicon-96x96.png" | "apple-touch-icon.png"
                )
        })
    }
}

/// A portal deep link (a client-side route), as opposed to `/portal/api/**`.
/// The distinction matters because callers use `asset_path` to decide whether a
/// request is a static asset and may skip admission accounting for those.
fn portal_section(path: &str) -> bool {
    path.starts_with("/portal/") && !path.starts_with("/portal/api/")
}

/// The operator console is not part of this build; only the user portal is.
/// Old bookmarks under `/admin/**` still resolve, because people and scripts
/// have them, but the HTML surface they reached no longer exists. `/admin/api`
/// is handled earlier in the request pipeline and never sees this redirect.
pub(crate) fn portal_redirect(parts: &Parts) -> Option<Response<Bytes>> {
    if !is_read(parts) {
        return None;
    }
    let path = parts.uri.path();
    if path != "/admin" && !path.starts_with("/admin/") {
        return None;
    }
    let mut response = Response::new(Bytes::new());
    *response.status_mut() = StatusCode::FOUND;
    response
        .headers_mut()
        .insert(http::header::LOCATION, HeaderValue::from_static("/portal"));
    response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    Some(response)
}

/// Only `GET` and `HEAD` reach the embedded assets; anything else is API traffic.
fn is_read(parts: &Parts) -> bool {
    parts.method == Method::GET || parts.method == Method::HEAD
}

pub(crate) fn serve(parts: &Parts) -> Option<Response<Bytes>> {
    let asset = asset_path(parts)?;
    if asset == "build-info.js" {
        return Some(build_info(parts.method == Method::HEAD));
    }
    if WebAssets::get("index.html").is_none() {
        return Some(text(
            StatusCode::NOT_FOUND,
            "web assets are not embedded; run `pnpm build` in console/ and rebuild gproxy",
        ));
    }
    let Some(content) = WebAssets::get(asset) else {
        return Some(text(StatusCode::NOT_FOUND, "not found"));
    };
    let mut response = Response::new(if parts.method == Method::HEAD {
        Bytes::new()
    } else if asset == "index.html" {
        Bytes::from(
            String::from_utf8_lossy(&content.data).replace(
                "</head>",
                "<script src=\"/build-info.js\"></script><script src=\"/announcements.js\"></script></head>",
            ),
        )
    } else {
        Bytes::from(content.data.into_owned())
    });
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_str(
            mime_guess::from_path(asset)
                .first_raw()
                .unwrap_or("application/octet-stream"),
        )
        .expect("MIME types are valid header values"),
    );
    response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        if asset == "index.html" {
            HeaderValue::from_static("no-cache")
        } else {
            HeaderValue::from_static("public, max-age=31536000, immutable")
        },
    );
    Some(response)
}

fn build_info(head: bool) -> Response<Bytes> {
    let value = serde_json::json!({
        "version": crate::BUILD_VERSION,
        "channel": crate::BUILD_CHANNEL,
        "buildHash": crate::BUILD_HASH,
        "installationKind": crate::installation_kind(),
    });
    let body = format!("globalThis.__GPROXY_BUILD_INFO__ = {value};\n");
    let mut response = Response::new(if head {
        Bytes::new()
    } else {
        Bytes::from(body)
    });
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    response
}

fn text(status: StatusCode, body: &'static str) -> Response<Bytes> {
    let mut response = Response::new(Bytes::from_static(body.as_bytes()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(method: Method, path: &str) -> Parts {
        http::Request::builder()
            .method(method)
            .uri(path)
            .body(())
            .expect("request")
            .into_parts()
            .0
    }

    #[test]
    fn portal_deep_links_serve_the_application_shell() {
        for path in ["/", "/portal", "/portal/", "/portal/keys", "/portal/usage"] {
            assert_eq!(
                asset_path(&parts(Method::GET, path)),
                Some("index.html"),
                "{path} should serve the shell"
            );
        }
    }

    #[test]
    fn hashed_assets_and_icons_are_served_but_arbitrary_paths_are_not() {
        assert_eq!(
            asset_path(&parts(Method::GET, "/assets/index-abc.js")),
            Some("assets/index-abc.js")
        );
        assert_eq!(
            asset_path(&parts(Method::GET, "/favicon.ico")),
            Some("favicon.ico")
        );
        // Gateway traffic and unknown admin pages must not fall back to the shell.
        assert_eq!(asset_path(&parts(Method::GET, "/v1/models")), None);
        assert_eq!(asset_path(&parts(Method::GET, "/admin")), None);
        assert_eq!(asset_path(&parts(Method::GET, "/portal/api/keys")), None);
    }

    #[test]
    fn only_read_methods_reach_the_embedded_assets() {
        assert_eq!(asset_path(&parts(Method::POST, "/portal")), None);
        assert_eq!(asset_path(&parts(Method::DELETE, "/portal/keys")), None);
    }

    #[test]
    fn admin_page_requests_redirect_to_the_portal() {
        for path in [
            "/admin",
            "/admin/",
            "/admin/providers",
            "/admin/identity/users/1",
        ] {
            let response = portal_redirect(&parts(Method::GET, path)).expect("redirect");
            assert_eq!(response.status(), StatusCode::FOUND, "{path}");
            assert_eq!(response.headers()[http::header::LOCATION], "/portal");
        }
        // `/admin/api` is dispatched before this helper, but guard it anyway: an
        // API response must never be a redirect to the portal.
        assert!(portal_redirect(&parts(Method::GET, "/portal/keys")).is_none());
        assert!(portal_redirect(&parts(Method::POST, "/admin/providers")).is_none());
    }
}
