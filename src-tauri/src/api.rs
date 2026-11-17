//! 业务命令：登录 / 列表 / 详情 / 播放 / 互动。

use crate::net::{edge, BASE};
use crate::session::SessionStore;
use serde_json::{json, Value};
use tauri::State;

pub struct Api {
    pub session: SessionStore,
}

impl Api {
    pub fn new() -> Self {
        Self {
            session: SessionStore::new(),
        }
    }

    fn token(&self) -> Option<String> {
        if cfg!(target_os = "android") { None } else { self.session.token() }
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let token = self.token();
        edge("GET", path, None, token.as_deref()).await
    }

    async fn authed(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        let token = self
            .token()
            .ok_or_else(|| "请先登录".to_string())?;
        edge(method, path, body, Some(&token)).await
    }
}

/// 诊断用：把前端自检结果写到应用数据目录 debug.log
#[tauri::command]
pub async fn debug_log(app: tauri::AppHandle, msg: String) -> Result<(), String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("debug.log");
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", msg);
    }
    Ok(())
}

/* ───────────────────────── 会话 ───────────────────────── */

#[tauri::command]
pub async fn app_boot(state: State<'_, Api>) -> Result<Value, String> {
    let user = if cfg!(target_os = "android") { Value::Null } else { state.session.user() };
    Ok(json!({
        "user": user,
        "logged_in": state.token().is_some(),
        "base": BASE,
    }))
}

#[tauri::command]
pub async fn api_login(
    state: State<'_, Api>,
    username: String,
    password: String,
) -> Result<Value, String> {
    if cfg!(target_os = "android") { return Err("Android 版本不支持账号功能".to_string()); }
    let data = edge(
        "POST",
        "/api/account/loginByPassword",
        Some(json!({ "username": username, "password": password })),
        None,
    )
    .await?;
    let token = data
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "登录返回缺少 token".to_string())?
        .to_string();
    let user = data.get("user").cloned().unwrap_or(Value::Null);
    state.session.set(token, user.clone());
    Ok(json!({ "user": user }))
}

#[tauri::command]
pub async fn api_register(
    state: State<'_, Api>,
    username: String,
    password: String,
    email: String,
) -> Result<Value, String> {
    if cfg!(target_os = "android") { return Err("Android 版本不支持账号功能".to_string()); }
    let mut body = json!({
        "username": username,
        "password": password,
        "confirm_password": password,
    });
    if !email.trim().is_empty() {
        body["email"] = json!(email);
    }
    let data = edge("POST", "/api/account/registerByPassword", Some(body), None).await?;
    let token = data
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "注册返回缺少 token".to_string())?
        .to_string();
    let user = data.get("user").cloned().unwrap_or(Value::Null);
    state.session.set(token, user.clone());
    Ok(json!({ "user": user }))
}

#[tauri::command]
pub async fn api_logout(state: State<'_, Api>) -> Result<Value, String> {
    let token = state.token();
    let _ = edge("POST", "/api/account/logout", None, token.as_deref()).await;
    state.session.clear();
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub async fn api_me(state: State<'_, Api>) -> Result<Value, String> {
    let me = state.get("/api/account/me").await?;
    if let Some(user) = state.session.token() {
        state.session.set(user, me.clone());
    }
    Ok(me)
}

/* ───────────────────────── 列表 ───────────────────────── */

#[tauri::command]
pub async fn api_feed(state: State<'_, Api>, page: u32, size: u32) -> Result<Value, String> {
    state
        .get(&format!("/api/videos?page={page}&size={size}"))
        .await
}

#[tauri::command]
pub async fn api_category(
    state: State<'_, Api>,
    cate: String,
    page: u32,
    size: u32,
) -> Result<Value, String> {
    state
        .get(&format!(
            "/api/videos/category/{cate}?page={page}&size={size}"
        ))
        .await
}

#[tauri::command]
pub async fn api_rank(state: State<'_, Api>, kind: String) -> Result<Value, String> {
    let kind = match kind.as_str() {
        "recommend" | "potential" => kind,
        _ => "hot".to_string(),
    };
    state.get(&format!("/api/ranks/{kind}")).await
}

#[tauri::command]
pub async fn api_search(
    state: State<'_, Api>,
    q: String,
    page: u32,
    size: u32,
) -> Result<Value, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
    state
        .get(&format!("/api/search?q={encoded}&page={page}&size={size}"))
        .await
}

#[tauri::command]
pub async fn api_posts(state: State<'_, Api>, page: u32, size: u32) -> Result<Value, String> {
    state
        .get(&format!("/api/posts?page={page}&size={size}"))
        .await
}

#[tauri::command]
pub async fn api_post(state: State<'_, Api>, id: i64) -> Result<Value, String> {
    state.get(&format!("/api/posts/{id}")).await
}

/* ───────────────────────── 详情 / 播放 ───────────────────────── */

#[tauri::command]
pub async fn api_video(state: State<'_, Api>, id: i64) -> Result<Value, String> {
    state.get(&format!("/api/videos/{id}")).await
}

#[tauri::command]
pub async fn api_post_play(state: State<'_, Api>, id: i64) -> Result<Value, String> {
    let token = state.token();
    let data = edge(
        "POST",
        &format!("/api/posts/{id}/playback"),
        Some(json!({})),
        token.as_deref(),
    )
    .await?;
    normalize_post_playback(data, id)
}

#[tauri::command]
pub async fn api_play(state: State<'_, Api>, id: i64, ep: u32) -> Result<Value, String> {
    let token = state.token();
    let data = edge(
        "POST",
        &format!("/api/videos/{id}/episodes/{ep}/playback"),
        Some(json!({})),
        token.as_deref(),
    )
    .await?;
    normalize_playback(data, id, ep)
}

fn playback_number(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}

fn playback_url(value: &Value) -> Option<&str> {
    let value = value.as_str()?;
    let url = url::Url::parse(value).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(value)
}

fn normalize_playback(data: Value, id: i64, ep: u32) -> Result<Value, String> {
    if data.get("manifest_method").and_then(Value::as_str) != Some("GET")
        || data.get("id").and_then(playback_number) != Some(id)
        || data.get("ep").and_then(playback_number) != Some(i64::from(ep))
    {
        return Err("上游返回的播放信息无效".to_string());
    }
    let url = data.pointer("/streams/main")
        .and_then(playback_url)
        .ok_or_else(|| "暂无播放地址".to_string())?;
    let hevc = data.pointer("/streams/hevc").and_then(Value::as_str).unwrap_or("");
    Ok(json!({ "id": id, "ep_num": ep, "video_url": url, "video_url_hevc": hevc }))
}

fn normalize_post_playback(data: Value, id: i64) -> Result<Value, String> {
    if data.get("manifest_method").and_then(Value::as_str) != Some("GET")
        || data.get("id").and_then(playback_number) != Some(id)
    {
        return Err("上游返回的播放信息无效".to_string());
    }
    let videos = data.get("videos").and_then(Value::as_array)
        .ok_or_else(|| "上游返回的播放信息无效".to_string())?;
    let mut result = Vec::with_capacity(videos.len());
    for video in videos {
        let index = video.get("index").and_then(Value::as_u64)
            .ok_or_else(|| "上游返回的播放信息无效".to_string())?;
        let url = video.get("url").and_then(playback_url)
            .ok_or_else(|| "上游返回的播放信息无效".to_string())?;
        result.push(json!({ "index": index, "url": url }));
    }
    Ok(json!({ "id": id, "videos": result }))
}

#[cfg(test)]
mod playback_tests {
    use super::*;

    #[test]
    fn accepts_current_playback_response() {
        let response = json!({
            "manifest_method": "GET", "id": 12, "ep": 3,
            "streams": { "main": "https://example.com/episode.m3u8", "hevc": "" }
        });
        let play = normalize_playback(response, 12, 3).unwrap();
        assert_eq!(play["video_url"], "https://example.com/episode.m3u8");
        assert_eq!(play["ep_num"], 3);
    }

    #[test]
    fn rejects_wrong_episode_and_missing_stream() {
        let response = json!({
            "manifest_method": "GET", "id": 12, "ep": 2,
            "streams": { "main": "https://example.com/episode.m3u8" }
        });
        assert!(normalize_playback(response, 12, 3).is_err());
        let response = json!({ "manifest_method": "GET", "id": 12, "ep": 3, "streams": {} });
        assert!(normalize_playback(response, 12, 3).is_err());
    }

    #[test]
    fn accepts_post_videos_and_picture_only_posts() {
        let response = json!({
            "manifest_method": "GET", "id": "67",
            "videos": [
                { "index": 0, "url": "https://example.com/first.m3u8" },
                { "index": 1, "url": "https://example.com/second.m3u8" }
            ]
        });
        let play = normalize_post_playback(response, 67).unwrap();
        assert_eq!(play["videos"].as_array().unwrap().len(), 2);
        assert_eq!(play["videos"][1]["index"], 1);
        let response = json!({ "manifest_method": "GET", "id": "621", "videos": [] });
        assert_eq!(normalize_post_playback(response, 621).unwrap()["videos"], json!([]));
    }

    #[tokio::test]
    #[ignore = "requires the live upstream site and media CDN"]
    async fn related_search_results_have_a_working_media_chain() {
        use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

        for (category, keyword) in [("ai-huanlian", "换脸"), ("ai-mogai", "魔改")] {
            let category_error = edge("GET", &format!("/api/videos/category/{category}?page=1&size=20"), None, None)
                .await.expect_err("removed category should report an error");
            assert!(category_error.starts_with("HTTP 410:"), "{keyword}: {category_error}");
            let query: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
            let search = edge("GET", &format!("/api/search?q={query}&page=1&size=20"), None, None)
                .await.expect("search request");
            let id = search.pointer("/items/0/id").and_then(playback_number).expect("search result id");
            let data = edge("POST", &format!("/api/videos/{id}/episodes/1/playback"), Some(json!({})), None)
                .await.expect("playback request");
            let play = normalize_playback(data, id, 1).expect("valid playback response");
            let url = play["video_url"].as_str().unwrap();
            let query = format!("u={}", utf8_percent_encode(url, NON_ALPHANUMERIC));
            let playlist = crate::media::handle("/hls", &query, "http").await;
            assert_eq!(playlist.status(), 200, "{keyword}: playlist");
            let body = String::from_utf8(playlist.body().clone()).unwrap();
            assert!(body.starts_with("#EXTM3U"), "{keyword}: invalid playlist");
            let key_query = body.lines().find(|line| line.starts_with("#EXT-X-KEY"))
                .and_then(|line| line.split("?u=").nth(1))
                .and_then(|value| value.split('"').next()).expect("rewritten key");
            let key = crate::media::handle("/seg", &format!("u={key_query}"), "http").await;
            assert_eq!(key.status(), 200, "{keyword}: key");
            assert_eq!(key.body().len(), 16, "{keyword}: key length");
            let segment_query = body.lines().find(|line| !line.starts_with('#') && line.contains("/seg?u="))
                .and_then(|line| line.split("?u=").nth(1)).expect("rewritten segment");
            let segment = crate::media::handle("/seg", &format!("u={segment_query}"), "http").await;
            assert_eq!(segment.status(), 200, "{keyword}: segment");
            assert!(segment.body().len() > 1000, "{keyword}: empty segment");
            println!("{keyword}: search, playback, playlist, key and segment succeeded");
        }
    }
}

/* ───────────────────────── 评论 / 互动 ───────────────────────── */

#[tauri::command]
pub async fn api_comments(
    state: State<'_, Api>,
    id: i64,
    ep: u32,
    page: u32,
    size: u32,
) -> Result<Value, String> {
    state
        .get(&format!(
            "/api/videos/{id}/comments?ep={ep}&page={page}&size={size}"
        ))
        .await
}

#[tauri::command]
pub async fn api_comment_send(
    state: State<'_, Api>,
    id: i64,
    ep: u32,
    text: String,
    reply_id: Option<i64>,
) -> Result<Value, String> {
    let mut body = json!({ "text": text });
    let path = match reply_id {
        Some(rid) if rid > 0 => {
            body["reply_id"] = json!(rid);
            format!("/api/videos/{id}/comments")
        }
        _ => format!("/api/videos/{id}/comments?ep={ep}"),
    };
    state.authed("POST", &path, Some(body)).await
}

#[tauri::command]
pub async fn api_like(
    state: State<'_, Api>,
    id: i64,
    ep: u32,
    on: bool,
) -> Result<Value, String> {
    let method = if on { "POST" } else { "DELETE" };
    state
        .authed(method, &format!("/api/videos/{id}/like?ep={ep}"), None)
        .await
}

#[tauri::command]
pub async fn api_collect(
    state: State<'_, Api>,
    id: i64,
    on: bool,
) -> Result<Value, String> {
    let method = if on { "POST" } else { "DELETE" };
    state
        .authed(method, &format!("/api/videos/{id}/collect"), None)
        .await
}

#[tauri::command]
pub async fn api_follow(
    state: State<'_, Api>,
    uid: i64,
    on: bool,
) -> Result<Value, String> {
    let method = if on { "POST" } else { "DELETE" };
    state
        .authed(method, &format!("/api/users/{uid}/follow"), None)
        .await
}

/* ───────────────────────── 本地缓存 ───────────────────────── */

#[tauri::command]
pub async fn cache_clear() -> Result<(), String> {
    crate::media::clear_caches();
    Ok(())
}

/* ───────────────────────── 吃瓜帖子互动 ───────────────────────── */

#[tauri::command]
pub async fn api_post_like(state: State<'_, Api>, id: i64, on: bool) -> Result<Value, String> {
    let method = if on { "POST" } else { "DELETE" };
    state
        .authed(method, &format!("/api/posts/{id}/like"), None)
        .await
}

#[tauri::command]
pub async fn api_post_collect(state: State<'_, Api>, id: i64, on: bool) -> Result<Value, String> {
    let method = if on { "POST" } else { "DELETE" };
    state
        .authed(method, &format!("/api/posts/{id}/collect"), None)
        .await
}

#[tauri::command]
pub async fn api_post_comments(
    state: State<'_, Api>,
    id: i64,
    page: u32,
    size: u32,
) -> Result<Value, String> {
    state
        .get(&format!("/api/posts/{id}/comments?page={page}&size={size}"))
        .await
}
