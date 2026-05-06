use rand::RngExt;
use reqwest::header;

use crate::consts::CHAT_URL;

fn random_queue_code() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_";
    let mut rng = rand::rng();
    (0..32)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

async fn enter_queue(client: &reqwest::Client, token: &str) -> reqwest::Result<String> {
    let queue_code = random_queue_code();

    client
        .get(format!(
            "{CHAT_URL}/ms-api/mei-wei-bu-yong-deng?queue_code={queue_code}"
        ))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;

    Ok(queue_code)
}

pub async fn request_chat(
    client: &reqwest::Client,
    token: &str,
    model: &str,
    messages: &serde_json::Value,
    tools: Vec<serde_json::Value>,
) -> reqwest::Result<reqwest::Response> {
    let queue_code = enter_queue(client, token).await?;

    let mut payload = serde_json::json!({
        "messages": messages,
        "queue_code": queue_code,
        "model": model,
        "stream": true,
        "with_search": false,
    });

    if !tools.is_empty() {
        payload["tools"] = serde_json::Value::Array(tools);
    }

    client
        .post(format!("{CHAT_URL}/ms-api/chat-messages"))
        .bearer_auth(token)
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await?
        .error_for_status()
}
