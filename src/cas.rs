use std::sync::LazyLock;

use aes::Aes128;
use base64::{Engine, engine::general_purpose};
use ecb::{
    Encryptor,
    cipher::{BlockModeEncrypt, KeyInit, block_padding::Pkcs7},
};
use regex::Regex;

use crate::consts::{CAS_URL, CHAT_URL};

static RE_CRYPTO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<p id="login-croypto">(.+?)</p>"#).unwrap());
static RE_FLOWKEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<p id="login-page-flowkey">(.+?)</p>"#).unwrap());
static RE_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<div\s+class="alert alert-danger"\s+id="login-error-msg">\s*<span>([^<]+?)</span>\s*</div>"#,
    )
    .unwrap()
});
static RE_TICKET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"ticket=([^&]+)").unwrap());

pub async fn login(username: &str, password: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // Step 1: GET CAS login page
    let resp = client.get(format!("{CAS_URL}/cas/login")).send().await?;
    let html = resp.text().await?;

    let crypto = RE_CRYPTO
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or("Failed to extract crypto param")?;

    let flow_key = RE_FLOWKEY
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or("Failed to extract flowkey param")?;

    // Step 2: AES-ECB encrypt password and captcha
    let key_bytes = general_purpose::STANDARD.decode(crypto)?;
    let encrypted_pwd = aes_ecb_encrypt(&key_bytes, password.as_bytes())?;
    let encrypted_captcha = aes_ecb_encrypt(&key_bytes, b"{}")?;

    // Step 3: POST login form
    let login_resp = client
        .post(format!("{CAS_URL}/cas/login"))
        .form(&[
            ("type", "UsernamePassword"),
            ("_eventId", "submit"),
            ("croypto", crypto),
            ("username", username),
            ("password", &encrypted_pwd),
            ("captcha_payload", &encrypted_captcha),
            ("execution", flow_key),
        ])
        .send()
        .await?;

    if login_resp.status().as_u16() != 302 {
        let body = login_resp.text().await?;
        let err_msg = RE_ERROR
            .captures(&body)
            .and_then(|c| c.get(1))
            .map_or("Unknown login error", |m| m.as_str());
        return Err(format!("Login failed: {err_msg}").into());
    }

    // Step 4: Get service ticket
    let ticket_url = format!(
        "{CAS_URL}/cas/login?service={}",
        url::form_urlencoded::byte_serialize(CHAT_URL.as_bytes()).collect::<String>()
    );

    let ticket_resp = client.get(&ticket_url).send().await?;

    let location = ticket_resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or("Failed to get CAS ticket redirect")?;

    let ticket = RE_TICKET
        .captures(location)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| format!("Failed to extract CAS ticket from: {location}"))?;

    // Step 5: Exchange ticket for USTChat token
    let token_resp = client
        .post(format!("{CHAT_URL}/ms-api/cas"))
        .json(&serde_json::json!({"ticket": ticket}))
        .send()
        .await?
        .error_for_status()?;

    let token_data: serde_json::Value = token_resp.json().await?;

    token_data["data"]["token"]
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| "Token not found in response".into())
}

fn aes_ecb_encrypt(key: &[u8], plaintext: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let cipher = Encryptor::<Aes128>::new_from_slice(key)?;

    let ciphertext_vec = cipher.encrypt_padded_vec::<Pkcs7>(plaintext);

    Ok(general_purpose::STANDARD.encode(ciphertext_vec))
}
