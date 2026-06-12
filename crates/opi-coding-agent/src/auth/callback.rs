use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct CallbackResult {
    pub code: String,
    pub state: String,
}

pub async fn start_callback_server(expected_state: String) -> Result<CallbackResult, String> {
    let listener = TcpListener::bind("127.0.0.1:1455").await.map_err(|e| {
        format!("Failed to bind to loopback port 1455: {e}. Is another instance running?")
    })?;

    let fut = async {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|e| format!("Failed to accept connection: {e}"))?;

            let mut buf = vec![0u8; 4096];
            let n = match socket.read(&mut buf).await {
                Ok(0) => continue,
                Ok(n) => n,
                Err(e) => return Err(format!("Failed to read request: {e}")),
            };
            let request_str = String::from_utf8_lossy(&buf[..n]);

            let first_line = match request_str.lines().next() {
                Some(line) => line,
                None => continue,
            };

            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 2 || parts[0] != "GET" {
                let response = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
                continue;
            }

            let path_and_query = parts[1];
            if !path_and_query.starts_with("/auth/callback") {
                let response = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
                continue;
            }

            let query_str = match path_and_query.split_once('?') {
                Some((_, q)) => q,
                None => {
                    let html = error_html("Missing query parameters in callback");
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return Err("Missing query parameters in callback".to_string());
                }
            };

            let params = parse_query_string(query_str);

            if let Some(error) = params.get("error") {
                let desc = params
                    .get("error_description")
                    .cloned()
                    .unwrap_or_else(|| error.clone());
                let html = error_html(&desc);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = socket.write_all(response.as_bytes()).await;
                return Err(format!("Auth error from callback: {desc}"));
            }

            let state = match params.get("state") {
                Some(s) => s,
                None => {
                    let html = error_html("Missing state parameter");
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return Err("Missing state parameter".to_string());
                }
            };

            if state != &expected_state {
                let html = error_html("State parameter mismatch (CSRF protection)");
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = socket.write_all(response.as_bytes()).await;
                return Err("CSRF state mismatch".to_string());
            }

            let code = match params.get("code") {
                Some(c) => c,
                None => {
                    let html = error_html("Missing code parameter");
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return Err("Missing code parameter".to_string());
                }
            };

            let html = success_html();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = socket.write_all(response.as_bytes()).await;

            return Ok(CallbackResult {
                code: code.clone(),
                state: state.clone(),
            });
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(15 * 60), fut).await {
        Ok(result) => result,
        Err(_) => Err("Authentication timed out after 15 minutes".to_string()),
    }
}

fn parse_query_string(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            params.insert(k.to_string(), percent_decode(v));
        }
    }
    params
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

fn success_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Opi Login Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: #0d1117;
            color: #c9d1d9;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
            padding: 30px;
            background-color: #161b22;
            border: 1px solid #30363d;
            border-radius: 8px;
            box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
            max-width: 400px;
        }
        h1 {
            color: #2ea44f;
            margin-bottom: 15px;
        }
        p {
            font-size: 16px;
            line-height: 1.5;
        }
        .footer {
            margin-top: 20px;
            font-size: 12px;
            color: #8b949e;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Login Successful</h1>
        <p>You have successfully logged in to OpenAI Codex for <strong>opi</strong>.</p>
        <p>You can close this browser window and return to your terminal.</p>
        <div class="footer">Opi AI Coding Agent</div>
    </div>
</body>
</html>"#.to_string()
}

fn error_html(details: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Opi Login Failed</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: #0d1117;
            color: #c9d1d9;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
        }}
        .container {{
            text-align: center;
            padding: 30px;
            background-color: #161b22;
            border: 1px solid #f85149;
            border-radius: 8px;
            box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
            max-width: 400px;
        }}
        h1 {{
            color: #f85149;
            margin-bottom: 15px;
        }}
        p {{
            font-size: 16px;
            line-height: 1.5;
        }}
        .error-details {{
            background-color: #21262d;
            border: 1px solid #30363d;
            padding: 10px;
            border-radius: 4px;
            font-family: monospace;
            text-align: left;
            margin: 15px 0;
            overflow-x: auto;
            color: #ff7b72;
        }}
        .footer {{
            margin-top: 20px;
            font-size: 12px;
            color: #8b949e;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Login Failed</h1>
        <p>An error occurred during authentication:</p>
        <div class="error-details">{}</div>
        <p>Please try logging in again from your terminal.</p>
        <div class="footer">Opi AI Coding Agent</div>
    </div>
</body>
</html>"#,
        html_escape(details)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
