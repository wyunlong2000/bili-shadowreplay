use crate::account::Account;
use crate::errors::RecorderError;
use crate::utils::user_agent_generator;
use deno_core::JsRuntime;
use deno_core::RuntimeOptions;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep_until, Instant};
use url::Url;
use uuid::Uuid;

use super::response::DouyinRoomInfoResponse;

static DOUYIN_API_NEXT_REQUEST: OnceLock<Mutex<Instant>> = OnceLock::new();

/// 为进程内的抖音监测和元数据请求分配全局请求时隙。
async fn wait_for_douyin_api_slot() {
    let next_request = DOUYIN_API_NEXT_REQUEST.get_or_init(|| Mutex::new(Instant::now()));
    let mut next_request = next_request.lock().await;
    if *next_request > Instant::now() {
        sleep_until(*next_request).await;
    }
    *next_request = Instant::now() + Duration::from_secs(30);
}

#[derive(Debug, Clone)]
pub struct DouyinBasicRoomInfo {
    pub room_id_str: String,
    pub room_title: String,
    pub cover: Option<String>,
    pub status: i64,
    pub hls_url: String,
    pub stream_data: String,
    // user related
    pub user_name: String,
    pub user_avatar: String,
    pub sec_user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DouyinIdentifier {
    Room(String),
    Profile(String),
}

#[derive(Debug, Clone, Default)]
pub struct DouyinProfileInfo {
    pub sec_uid: String,
    pub nickname: String,
    pub avatar: String,
}

#[derive(Default)]
pub struct DouyinApiSession {
    pub cookies: String,
    last_empty_refresh: Option<Instant>,
    web_id: String,
    uifid: String,
    verify_fp: String,
}

impl DouyinApiSession {
    /// 使用已保存的抖音账号 Cookie 创建接口会话。
    pub fn new(cookies: String) -> Self {
        let web_id = cookie_value(&cookies, "webid")
            .or_else(|| cookie_value(&cookies, "s_v_web_id"))
            .map(str::to_string)
            .unwrap_or_else(generate_web_id);
        let verify_fp = cookie_value(&cookies, "verifyFp")
            .or_else(|| cookie_value(&cookies, "verify_fp"))
            .map(str::to_string)
            .unwrap_or_else(generate_verify_fp);
        Self {
            cookies,
            last_empty_refresh: None,
            web_id,
            uifid: generate_uifid(),
            verify_fp,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DouyinProfileLiveInfo {
    pub live: bool,
    pub web_rid: Option<String>,
    pub nickname: String,
    pub avatar: String,
}

#[derive(Debug, Deserialize)]
struct DouyinPublicProfileResponse {
    status_code: Option<i64>,
    user_info: Option<DouyinPublicProfileUser>,
}

#[derive(Debug, Deserialize)]
struct DouyinPublicProfileUser {
    sec_uid: Option<String>,
    nickname: Option<String>,
    avatar_thumb: Option<DouyinPublicProfileAvatar>,
}

#[derive(Debug, Deserialize)]
struct DouyinPublicProfileAvatar {
    url_list: Option<Vec<String>>,
}

/// 判断域名是否属于抖音主页或短链服务。
fn is_douyin_host(host: &str) -> bool {
    host == "douyin.com"
        || host.ends_with(".douyin.com")
        || host == "iesdouyin.com"
        || host.ends_with(".iesdouyin.com")
}

/// 校验抖音 sec_uid 的前缀、长度和字符集。
fn is_sec_uid(value: &str) -> bool {
    value.starts_with("MS4wLjAB")
        && value.len() >= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// 从抖音主页或直播间链接中提取稳定标识。
fn identify_douyin_url(url: &Url) -> Option<DouyinIdentifier> {
    let host = url.host_str()?;
    if !is_douyin_host(host) {
        return None;
    }

    let segments = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if host == "live.douyin.com" {
        if let Some(room_id) = segments.first().filter(|value| value.bytes().all(|b| b.is_ascii_digit())) {
            return Some(DouyinIdentifier::Room((*room_id).to_string()));
        }
    }

    for pair in segments.windows(2) {
        if pair[0] == "user" && is_sec_uid(pair[1]) {
            return Some(DouyinIdentifier::Profile(pair[1].to_string()));
        }
    }
    None
}

/// 解析抖音主播输入，限制短链跳转仅访问抖音域名。
pub async fn normalize_douyin_identifier(
    input: &str,
) -> Result<DouyinIdentifier, RecorderError> {
    let input = input.trim();
    if input.bytes().all(|byte| byte.is_ascii_digit()) && !input.is_empty() {
        return Ok(DouyinIdentifier::Room(input.to_string()));
    }
    if is_sec_uid(input) {
        return Ok(DouyinIdentifier::Profile(input.to_string()));
    }

    let input = input.strip_prefix("bsr://").map_or_else(
        || input.to_string(),
        |value| format!("https://{value}"),
    );
    let mut url = Url::parse(&input).map_err(|_| RecorderError::InvalidValue)?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(RecorderError::InvalidValue);
    }
    if !url.host_str().is_some_and(is_douyin_host) {
        return Err(RecorderError::InvalidValue);
    }
    if let Some(identifier) = identify_douyin_url(&url) {
        return Ok(identifier);
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                return attempt.stop();
            }
            if attempt.url().host_str().is_some_and(is_douyin_host) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()?;
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://www.douyin.com/"),
    );
    wait_for_douyin_api_slot().await;
    let response = client.get(url.clone()).headers(headers).send().await?;
    url = response.url().clone();
    if !response.status().is_success() {
        return Err(RecorderError::InvalidValue);
    }
    identify_douyin_url(&url).ok_or(RecorderError::InvalidValue)
}

/// 查询公开抖音主页资料并校验主播标识。
pub async fn get_public_profile(
    client: &Client,
    sec_uid: &str,
) -> Result<DouyinProfileInfo, RecorderError> {
    if !is_sec_uid(sec_uid) {
        return Err(RecorderError::InvalidValue);
    }
    let url = Url::parse_with_params(
        "https://www.iesdouyin.com/web/api/v2/user/info/",
        &[("sec_uid", sec_uid)],
    )
    .map_err(|_| RecorderError::InvalidValue)?;
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://www.iesdouyin.com/"),
    );
    wait_for_douyin_api_slot().await;
    let response = client.get(url).headers(headers).send().await?;
    if !response.status().is_success() {
        return Err(RecorderError::InvalidValue);
    }
    let profile = response
        .json::<DouyinPublicProfileResponse>()
        .await
        .map_err(|_| RecorderError::InvalidResponse)?;
    let user = profile
        .user_info
        .filter(|_| profile.status_code == Some(0))
        .ok_or(RecorderError::InvalidValue)?;
    let nickname = user.nickname.unwrap_or_default().trim().to_string();
    let actual_sec_uid = user.sec_uid.unwrap_or_default();
    if nickname.is_empty() || actual_sec_uid != sec_uid {
        return Err(RecorderError::InvalidValue);
    }
    let avatar = user
        .avatar_thumb
        .and_then(|avatar| avatar.url_list)
        .and_then(|urls| urls.into_iter().next())
        .unwrap_or_default();

    Ok(DouyinProfileInfo {
        sec_uid: actual_sec_uid,
        nickname,
        avatar,
    })
}

/// 确保抖音接口会话包含 ttwid Cookie。
pub async fn ensure_douyin_session(
    client: &Client,
    session: &mut DouyinApiSession,
) -> Result<(), RecorderError> {
    if cookie_value(&session.cookies, "ttwid").is_some() {
        return Ok(());
    }
    refresh_ttwid(client, session).await
}

/// 按 sec_uid 查询主页开播状态，并解析开播时的有效直播间号。
pub async fn check_user_live_by_profile_api(
    client: &Client,
    session: &mut DouyinApiSession,
    sec_user_id: &str,
) -> Result<DouyinProfileLiveInfo, RecorderError> {
    if !is_sec_uid(sec_user_id) {
        return Err(RecorderError::InvalidValue);
    }
    ensure_douyin_session(client, session).await?;

    let mut response_body = request_profile_other(client, session, sec_user_id).await?;
    if response_body.trim().is_empty() {
        let can_refresh = session
            .last_empty_refresh
            .is_none_or(|last_refresh| last_refresh.elapsed() >= Duration::from_secs(60));
        if !can_refresh {
            return Err(RecorderError::ApiError {
                error: "抖音主页接口返回空响应".to_string(),
            });
        }
        refresh_ttwid(client, session).await?;
        session.last_empty_refresh = Some(Instant::now());
        response_body = request_profile_other(client, session, sec_user_id).await?;
    }
    if response_body.trim().is_empty() {
        return Err(RecorderError::ApiError {
            error: "抖音主页接口重建会话后仍返回空响应".to_string(),
        });
    }

    let response: serde_json::Value = serde_json::from_str(&response_body).map_err(|_| {
        let body_lower = response_body.to_ascii_lowercase();
        if body_lower.contains("captcha")
            || body_lower.contains("verify_page")
            || body_lower.contains("security_verify")
            || body_lower.contains("/verify/")
            || response_body.contains("验证码")
            || response_body.contains("安全验证")
        {
            RecorderError::ApiError {
                error: "抖音主页接口触发安全验证".to_string(),
            }
        } else {
            RecorderError::InvalidResponse
        }
    })?;
    if response
        .get("status_code")
        .and_then(serde_json::Value::as_i64)
        != Some(0)
    {
        return Err(RecorderError::ApiError {
            error: "抖音主页接口返回错误状态".to_string(),
        });
    }
    let user = response
        .get("user")
        .ok_or(RecorderError::InvalidResponse)?;
    let actual_sec_uid = user
        .get("sec_uid")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if actual_sec_uid != sec_user_id {
        return Err(RecorderError::ApiError {
            error: "抖音主页接口返回的主播标识不匹配".to_string(),
        });
    }
    let live_status = user
        .get("live_status")
        .and_then(parse_integer_value)
        .ok_or(RecorderError::InvalidResponse)?;
    let live = live_status != 0;
    let nickname = user
        .get("nickname")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let avatar = ["avatar_300x300", "avatar_larger", "avatar_thumb"]
        .into_iter()
        .find_map(|key| {
            user.get(key)?
                .get("url_list")?
                .as_array()?
                .iter()
                .find_map(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    let web_rid = if live {
        Some(extract_profile_web_rid(user).ok_or_else(|| RecorderError::ApiError {
            error: "抖音主页接口确认开播，但未返回有效直播间号".to_string(),
        })?)
    } else {
        None
    };

    Ok(DouyinProfileLiveInfo {
        live,
        web_rid,
        nickname,
        avatar,
    })
}

/// 生成签名参数并请求抖音主页用户接口。
async fn request_profile_other(
    client: &Client,
    session: &DouyinApiSession,
    sec_user_id: &str,
) -> Result<String, RecorderError> {
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_str(&format!(
            "https://www.douyin.com/user/{sec_user_id}"
        ))
        .map_err(|_| RecorderError::InvalidValue)?,
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/155.0.0.0",
        ),
    );
    headers.insert(
        "Cookie",
        crate::utils::header_value("Cookie", &session.cookies)?,
    );
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let ms_token = generate_ms_token().await;
    let mut url = Url::parse("https://www.douyin.com/aweme/v1/web/user/profile/other/")
        .map_err(|_| RecorderError::InvalidValue)?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("device_platform", "webapp")
            .append_pair("aid", "6383")
            .append_pair("channel", "channel_pc_web")
            .append_pair("publish_video_strategy_type", "2")
            .append_pair("source", "channel_pc_web")
            .append_pair("sec_user_id", sec_user_id)
            .append_pair("personal_center_strategy", "1")
            .append_pair("profile_other_record_enable", "1")
            .append_pair("land_to", "1")
            .append_pair("update_version_code", "170400")
            .append_pair("pc_client_type", "1")
            .append_pair("pc_libra_divert", "Windows")
            .append_pair("support_h265", "1")
            .append_pair("support_dash", "1")
            .append_pair("version_code", "170400")
            .append_pair("version_name", "17.4.0")
            .append_pair("cookie_enabled", "true")
            .append_pair("screen_width", "2048")
            .append_pair("screen_height", "1281")
            .append_pair("browser_language", "zh-CN")
            .append_pair("browser_platform", "Win32")
            .append_pair("browser_name", "Edge")
            .append_pair("browser_version", "155.0.0.0")
            .append_pair("browser_online", "true")
            .append_pair("engine_name", "Blink")
            .append_pair("engine_version", "155.0.0.0")
            .append_pair("os_name", "Windows")
            .append_pair("os_version", "10")
            .append_pair("cpu_core_num", "14")
            .append_pair("device_memory", "32")
            .append_pair("platform", "PC")
            .append_pair("downlink", "10")
            .append_pair("effective_type", "4g")
            .append_pair("round_trip_time", "50")
            .append_pair("webid", &session.web_id)
            .append_pair("uifid", &session.uifid)
            .append_pair("verifyFp", &session.verify_fp)
            .append_pair("fp", &session.verify_fp)
            .append_pair("msToken", &ms_token);
    }
    let params = url.query().unwrap_or_default().to_string();
    let a_bogus = generate_a_bogus(&params, user_agent).await?;
    url.query_pairs_mut().append_pair("a_bogus", &a_bogus);

    wait_for_douyin_api_slot().await;
    let response = client.get(url).headers(headers).send().await?;
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Err(RecorderError::SecurityControlError);
    }
    if !response.status().is_success() {
        return Err(RecorderError::ApiError {
            error: format!("抖音主页接口请求失败：{}", response.status()),
        });
    }
    Ok(response.text().await?)
}

/// 请求抖音主页以刷新会话中的 ttwid Cookie。
async fn refresh_ttwid(
    client: &Client,
    session: &mut DouyinApiSession,
) -> Result<(), RecorderError> {
    let mut refreshed_cookies = session.cookies.clone();
    remove_cookie(&mut refreshed_cookies, "ttwid");
    for url in ["https://live.douyin.com/", "https://www.douyin.com/"] {
        let mut headers = generate_user_agent_header();
        headers.insert(
            "Referer",
            reqwest::header::HeaderValue::from_static("https://www.douyin.com/"),
        );
        wait_for_douyin_api_slot().await;
        let Ok(response) = client.get(url).headers(headers).send().await else {
            continue;
        };
        if let Some(value) = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|header| header.to_str().ok())
            .find_map(|header| {
                let cookie = header.split(';').next()?.trim();
                cookie.strip_prefix("ttwid=").map(str::to_string)
            })
        {
            set_cookie(&mut refreshed_cookies, "ttwid", &value);
            session.cookies = refreshed_cookies;
            return Ok(());
        }
    }
    Err(RecorderError::ApiError {
        error: "抖音会话初始化失败，请检查账号 Cookie".to_string(),
    })
}

/// 从 Cookie 字符串中查找指定名称的非空值。
fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies.split(';').find_map(|cookie| {
        let (key, value) = cookie.trim().split_once('=')?;
        (key == name && !value.trim().is_empty()).then_some(value.trim())
    })
}

/// 从 Cookie 字符串中移除指定名称的条目。
fn remove_cookie(cookies: &mut String, name: &str) {
    *cookies = cookies
        .split(';')
        .map(str::trim)
        .filter(|cookie| {
            cookie
                .split_once('=')
                .is_none_or(|(key, _)| key != name)
        })
        .filter(|cookie| !cookie.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
}

/// 在 Cookie 字符串中新增或替换指定条目。
fn set_cookie(cookies: &mut String, name: &str, value: &str) {
    remove_cookie(cookies, name);
    if !cookies.is_empty() {
        cookies.push_str("; ");
    }
    cookies.push_str(name);
    cookies.push('=');
    cookies.push_str(value);
}

/// 将 JSON 整数或数字字符串解析为有符号整数。
fn parse_integer_value(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

/// 从用户主页响应中提取可用于房间接口的 web_rid。
fn extract_profile_web_rid(user: &serde_json::Value) -> Option<String> {
    if let Some(web_rid) = user.get("web_rid").and_then(parse_room_identifier) {
        return Some(web_rid);
    }
    for key in ["room", "live_room"] {
        if let Some(room) = user.get(key) {
            if let Some(web_rid) = room
                .get("web_rid")
                .or_else(|| room.get("room_id"))
                .or_else(|| room.get("room_id_str"))
                .and_then(parse_room_identifier)
            {
                return Some(web_rid);
            }
        }
    }
    let room_data = user.get("room_data")?;
    let room_data = match room_data {
        serde_json::Value::String(value) => serde_json::from_str(value).ok()?,
        value => value.clone(),
    };
    room_data
        .get("owner")
        .and_then(|owner| owner.get("web_rid"))
        .and_then(parse_room_identifier)
        .or_else(|| room_data.get("web_rid").and_then(parse_room_identifier))
}

/// 将字符串或数字形式的直播间号规范为数字字符串。
fn parse_room_identifier(value: &serde_json::Value) -> Option<String> {
    let value = value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_u64().map(|value| value.to_string()))?;
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())).then_some(value)
}

fn setup_js_runtime() -> Result<JsRuntime, RecorderError> {
    // Create a new V8 runtime
    let mut runtime = JsRuntime::new(RuntimeOptions::default());

    // Add global CryptoJS object
    let crypto_js = include_str!("js/a_bogus.js");
    runtime
        .execute_script(
            "<a_bogus.js>",
            deno_core::FastString::from_static(crypto_js),
        )
        .map_err(|e| RecorderError::JsRuntimeError(format!("Failed to execute crypto-js: {e}")))?;
    Ok(runtime)
}

async fn generate_a_bogus(params: &str, user_agent: &str) -> Result<String, RecorderError> {
    let mut runtime = setup_js_runtime()?;
    // Call the get_wss_url function
    let sign_call = format!("generate_a_bogus(\"{params}\", \"{user_agent}\")");
    let result = runtime
        .execute_script("<sign_call>", deno_core::FastString::from(sign_call))
        .map_err(|e| RecorderError::JsRuntimeError(format!("Failed to execute JavaScript: {e}")))?;

    // Get the result from the V8 runtime
    let mut scope = runtime.handle_scope();
    let local = deno_core::v8::Local::new(&mut scope, result);
    let url = local
        .to_string(&mut scope)
        .ok_or_else(|| RecorderError::JsRuntimeError("a_bogus is not a string".to_string()))?
        .to_rust_string_lossy(&mut scope);
    Ok(url)
}

async fn generate_ms_token() -> String {
    // 生成随机 UUID 作为临时 msToken。
    let uuid = Uuid::new_v4();
    uuid.to_string()
}

/// 生成抖音主页接口使用的 webid。
fn generate_web_id() -> String {
    let value = Uuid::new_v4().as_u128() as u64;
    (1_000_000_000_000_000_000 + value % 9_000_000_000_000_000_000).to_string()
}

/// 生成抖音主页接口使用的设备标识。
fn generate_uifid() -> String {
    let value = Uuid::new_v4().simple().to_string();
    format!("{value}{value}{value}{value}")
}

/// 生成抖音主页接口使用的验证指纹。
fn generate_verify_fp() -> String {
    format!("verify_{}", Uuid::new_v4().simple())
}

pub fn generate_user_agent_header() -> reqwest::header::HeaderMap {
    let user_agent = user_agent_generator::UserAgentGenerator::new().generate(false);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "user-agent",
        user_agent
            .parse()
            .expect("generated user agent is a valid header value"),
    );
    headers
}

fn parse_web_room_info_response(
    data: DouyinRoomInfoResponse,
    sec_user_id: &str,
) -> Result<DouyinBasicRoomInfo, RecorderError> {
    let room = data
        .data
        .data
        .first()
        .ok_or_else(|| RecorderError::ApiError {
            error: "Douyin room info response did not include room data".to_string(),
        })?;

    let cover = room
        .cover
        .as_ref()
        .and_then(|cover| cover.url_list.first().cloned());
    let user_avatar = data
        .data
        .user
        .avatar_thumb
        .url_list
        .first()
        .cloned()
        .unwrap_or_default();

    Ok(DouyinBasicRoomInfo {
        room_id_str: room.id_str.clone(),
        sec_user_id: sec_user_id.to_string(),
        cover,
        room_title: room.title.clone(),
        user_name: data.data.user.nickname.clone(),
        user_avatar,
        status: data.data.room_status,
        hls_url: room
            .stream_url
            .as_ref()
            .map(|stream_url| stream_url.hls_pull_url.clone())
            .unwrap_or_default(),
        stream_data: room
            .stream_url
            .as_ref()
            .map(|s| s.live_core_sdk_data.pull_data.stream_data.clone())
            .unwrap_or_default(),
    })
}

pub async fn get_room_info(
    client: &Client,
    account: &Account,
    room_id: &str,
    sec_user_id: &str,
) -> Result<DouyinBasicRoomInfo, RecorderError> {
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://live.douyin.com/"),
    );
    headers.insert(
        "Cookie",
        crate::utils::header_value("Cookie", &account.cookies)?,
    );
    let ms_token = generate_ms_token().await;
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let params = format!(
            "aid=6383&app_name=douyin_web&live_id=1&device_platform=web&language=zh-CN&enter_from=web_live&cookie_enabled=true&screen_width=1920&screen_height=1080&browser_language=zh-CN&browser_platform=MacIntel&browser_name=Chrome&browser_version=122.0.0.0&web_rid={room_id}&ms_token={ms_token}");
    let a_bogus = generate_a_bogus(&params, user_agent).await?;
    // log::debug!("params: {params}");
    // log::debug!("user_agent: {user_agent}");
    // log::debug!("a_bogus: {a_bogus}");
    let url = format!(
            "https://live.douyin.com/webcast/room/web/enter/?aid=6383&app_name=douyin_web&live_id=1&device_platform=web&language=zh-CN&enter_from=web_live&cookie_enabled=true&screen_width=1920&screen_height=1080&browser_language=zh-CN&browser_platform=MacIntel&browser_name=Chrome&browser_version=122.0.0.0&web_rid={room_id}&ms_token={ms_token}&a_bogus={a_bogus}"
        );

    wait_for_douyin_api_slot().await;
    let resp = client.get(&url).headers(headers).send().await?;

    let status = resp.status();
    let text = resp.text().await?;

    if text.is_empty() {
        log::debug!("Empty room info response, trying H5 API");
        return get_room_info_h5(client, account, room_id, sec_user_id).await;
    }

    if status.is_success() {
        if let Ok(data) = serde_json::from_str::<DouyinRoomInfoResponse>(&text) {
            match parse_web_room_info_response(data, sec_user_id) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    log::warn!("Invalid douyin room info response: {e}; trying H5 API");
                    return get_room_info_h5(client, account, room_id, sec_user_id).await;
                }
            }
        }
        log::error!("Failed to parse room info response: {text}");
        return get_room_info_h5(client, account, room_id, sec_user_id).await;
    }

    log::error!("Failed to get room info: {status}");
    return get_room_info_h5(client, account, room_id, sec_user_id).await;
}

pub async fn get_room_info_h5(
    client: &Client,
    account: &Account,
    room_id: &str,
    sec_user_id: &str,
) -> Result<DouyinBasicRoomInfo, RecorderError> {
    // 参考biliup实现，构建完整的URL参数
    let room_id_str = room_id.to_string();
    // https://webcast.amemv.com/webcast/room/reflow/info/?type_id=0&live_id=1&version_code=99.99.99&app_id=1128&room_id=10000&sec_user_id=MS4wLjAB&aid=6383&device_platform=web&browser_language=zh-CN&browser_platform=Win32&browser_name=Mozilla&browser_version=5.0
    let url_params = [
        ("type_id", "0"),
        ("live_id", "1"),
        ("version_code", "99.99.99"),
        ("app_id", "1128"),
        ("room_id", &room_id_str),
        ("sec_user_id", sec_user_id),
        ("aid", "6383"),
        ("device_platform", "web"),
    ];

    // 构建URL
    let query_string = url_params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let url = format!("https://webcast.amemv.com/webcast/room/reflow/info/?{query_string}");

    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://live.douyin.com/"),
    );
    headers.insert(
        "Cookie",
        crate::utils::header_value("Cookie", &account.cookies)?,
    );

    wait_for_douyin_api_slot().await;
    let resp = client.get(&url).headers(headers).send().await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        // Try to parse as H5 response format
        if let Ok(h5_data) =
            serde_json::from_str::<super::response::DouyinH5RoomInfoResponse>(&text)
        {
            // Extract RoomBasicInfo from H5 response
            let room = &h5_data.data.room;
            let owner = &room.owner;

            let cover = room
                .cover
                .as_ref()
                .and_then(|c| c.url_list.first().cloned());
            let hls_url = room
                .stream_url
                .as_ref()
                .map(|s| s.hls_pull_url.clone())
                .unwrap_or_default();

            return Ok(DouyinBasicRoomInfo {
                room_id_str: room.id_str.clone(),
                room_title: room.title.clone(),
                cover,
                status: if room.status == 2 { 0 } else { 1 },
                hls_url,
                user_name: owner.nickname.clone(),
                user_avatar: owner
                    .avatar_thumb
                    .url_list
                    .first()
                    .cloned()
                    .unwrap_or_default(),
                sec_user_id: owner.sec_uid.clone(),
                stream_data: room
                    .stream_url
                    .as_ref()
                    .map(|s| s.live_core_sdk_data.pull_data.stream_data.clone())
                    .unwrap_or_default(),
            });
        }

        // If that fails, try to parse as a generic JSON to see what we got
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&text) {
            // Check if it's an error response
            if let Some(status_code) = json_value
                .get("status_code")
                .and_then(serde_json::Value::as_i64)
            {
                if status_code != 0 {
                    let error_msg = json_value
                        .get("data")
                        .and_then(|v| v.get("message").and_then(|v| v.as_str()))
                        .unwrap_or("Unknown error");

                    if status_code == 10011 {
                        return Err(RecorderError::ApiError {
                            error: error_msg.to_string(),
                        });
                    }

                    return Err(RecorderError::ApiError {
                        error: format!(
                            "API returned error status_code: {status_code} - {error_msg}"
                        ),
                    });
                }
            }

            // 检查是否是"invalid session"错误
            if let Some(status_message) = json_value.get("status_message").and_then(|v| v.as_str())
            {
                if status_message.contains("invalid session") {
                    return Err(RecorderError::ApiError { error:
                            "Invalid session - please check your cookies. Make sure you have valid sessionid, passport_csrf_token, and other authentication cookies from douyin.com".to_string(),
                        });
                }
            }

            return Err(RecorderError::ApiError {
                error: format!("Failed to parse h5 room info response: {text}"),
            });
        }
        log::error!("Failed to parse h5 room info response: {text}");
        return Err(RecorderError::ApiError {
            error: format!("Failed to parse h5 room info response: {text}"),
        });
    }

    log::error!("Failed to get h5 room info: {status}");
    Err(RecorderError::ApiError {
        error: format!("Failed to get h5 room info: {status} {text}"),
    })
}

pub async fn get_user_info(
    client: &Client,
    account: &Account,
) -> Result<super::response::User, RecorderError> {
    // Use the IM spotlight relation API to get user info
    let url = "https://www.douyin.com/aweme/v1/web/im/spotlight/relation/";
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://www.douyin.com/"),
    );
    headers.insert(
        "Cookie",
        crate::utils::header_value("Cookie", &account.cookies)?,
    );

    wait_for_douyin_api_slot().await;
    let resp = client.get(url).headers(headers).send().await?;

    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        if let Ok(data) = serde_json::from_str::<super::response::DouyinRelationResponse>(&text) {
            if data.status_code == 0 {
                let owner_sec_uid = &data.owner_sec_uid;

                // Find the user's own info in the followings list by matching sec_uid
                if let Some(followings) = &data.followings {
                    for following in followings {
                        if following.sec_uid == *owner_sec_uid {
                            let user = super::response::User {
                                id_str: following.uid.clone(),
                                sec_uid: following.sec_uid.clone(),
                                nickname: following.nickname.clone(),
                                avatar_thumb: following.avatar_thumb.clone(),
                                follow_info: super::response::FollowInfo::default(),
                                foreign_user: 0,
                                open_id_str: String::new(),
                            };
                            return Ok(user);
                        }
                    }
                }

                // If not found in followings, create a minimal user info from owner_sec_uid
                let user = super::response::User {
                    id_str: String::new(), // We don't have the numeric UID
                    sec_uid: owner_sec_uid.clone(),
                    nickname: "抖音用户".to_string(), // Default nickname
                    avatar_thumb: super::response::AvatarThumb { url_list: vec![] },
                    follow_info: super::response::FollowInfo::default(),
                    foreign_user: 0,
                    open_id_str: String::new(),
                };
                return Ok(user);
            }
        } else {
            log::error!("Failed to parse user info response: {text}");
            return Err(RecorderError::ApiError {
                error: format!("Failed to parse user info response: {text}"),
            });
        }
    }

    log::error!("Failed to get user info: {status}");

    Err(RecorderError::ApiError {
        error: format!("Failed to get user info: {status} {text}"),
    })
}

pub async fn get_room_owner_sec_uid(
    client: &Client,
    room_id: &str,
) -> Result<String, RecorderError> {
    let url = format!("https://live.douyin.com/{room_id}");
    let mut headers = generate_user_agent_header();
    headers.insert(
        "Referer",
        reqwest::header::HeaderValue::from_static("https://live.douyin.com/"),
    );
    wait_for_douyin_api_slot().await;
    let resp = client.get(url).headers(headers).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(RecorderError::ApiError {
            error: format!("Failed to get room owner sec uid: {status} {text}"),
        });
    }
    // match to get sec_uid from text like \"sec_uid\":\"MS4wLjABAAAAdFmmud36bynPjXOvoMjatb42856_zryHsGmlkpIECDA\"
    let sec_uid = Regex::new(r#"\\"sec_uid\\":\\"(.*?)\\""#)
        .expect("sec_uid regex is a valid literal")
        .captures(&text)
        .and_then(|c| c.get(1))
        .ok_or_else(|| RecorderError::ApiError {
            error: "Failed to find sec_uid in room page".to_string(),
        })?
        .as_str()
        .to_string();
    Ok(sec_uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platforms::douyin::response;

    fn web_room_response_with_room(room: Option<response::Daum>) -> DouyinRoomInfoResponse {
        DouyinRoomInfoResponse {
            data: response::Data {
                data: room.into_iter().collect(),
                room_status: 0,
                user: response::User {
                    nickname: "主播".to_string(),
                    avatar_thumb: response::AvatarThumb { url_list: vec![] },
                    ..Default::default()
                },
                ..Default::default()
            },
            status_code: 0,
            ..Default::default()
        }
    }

    #[test]
    fn parse_web_room_info_response_rejects_empty_room_data() {
        let err = parse_web_room_info_response(web_room_response_with_room(None), "sec_uid")
            .expect_err("empty room data should be rejected");

        assert!(matches!(err, RecorderError::ApiError { .. }));
    }

    #[test]
    fn parse_web_room_info_response_allows_empty_image_lists() {
        let room = response::Daum {
            id_str: "room_id".to_string(),
            title: "直播标题".to_string(),
            cover: Some(response::Cover { url_list: vec![] }),
            ..Default::default()
        };

        let info = parse_web_room_info_response(web_room_response_with_room(Some(room)), "sec_uid")
            .expect("empty image url lists should not fail room parsing");

        assert_eq!(info.room_id_str, "room_id");
        assert_eq!(info.room_title, "直播标题");
        assert_eq!(info.cover, None);
        assert_eq!(info.user_avatar, "");
    }

    #[test]
    fn extracts_sec_uid_from_room_page_html() {
        // Mirrors the escaped JSON fragment embedded in live.douyin.com pages.
        let html = r#"
            <script>window.__INIT__="{\"sec_uid\":\"MS4wLjABAAAAdFmmud36bynPjXOvoMjatb42856_zryHsGmlkpIECDA\",\"roomId\":\"123\"}"</script>
        "#;
        let sec_uid = Regex::new(r#"\\"sec_uid\\":\\"(.*?)\\""#)
            .expect("sec_uid regex is a valid literal")
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .expect("sec_uid should be extractable from fixture HTML");
        assert_eq!(
            sec_uid,
            "MS4wLjABAAAAdFmmud36bynPjXOvoMjatb42856_zryHsGmlkpIECDA"
        );
    }

    /// Optional live-network smoke test. Ignored in CI because Douyin pages
    /// frequently omit/change the embedded sec_uid payload without cookies.
    #[tokio::test]
    #[ignore = "live Douyin network smoke test; run manually with --ignored"]
    async fn test_get_room_owner_sec_uid_live() {
        let client = Client::new();
        let sec_uid = get_room_owner_sec_uid(&client, "200525029536")
            .await
            .unwrap();
        assert_eq!(
            sec_uid,
            "MS4wLjABAAAAdFmmud36bynPjXOvoMjatb42856_zryHsGmlkpIECDA"
        );
    }
}
