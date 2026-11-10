//! End-to-end test of the offline triage path: fixture file + fake Ollama.
//!
//! No real Ollama, no PAT, no external network: a std `TcpListener` serves the
//! canned `/api/chat` response on an ephemeral port.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

use kira::triage;

const FIXTURE_PATH: &str = "tests/fixtures/work_item_4821.json";

const FAKE_BODY: &str = r#"{"model":"qwen3.5:4b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia"},"done":true}"#;

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(headers_end) = find_subsequence(&raw, b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&raw[..headers_end]).to_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if raw.len() >= headers_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&raw).to_string()
}

fn spawn_fake_ollama() -> (String, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return None;
        };
        let request = read_http_request(&mut stream);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FAKE_BODY}",
            FAKE_BODY.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return None;
        }
        let _ = stream.flush();
        Some(request)
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn triage_file_prints_the_three_sections_with_the_model_text() {
    let (base_url, server) = spawn_fake_ollama();
    let output = triage::triage_file(&base_url, "qwen3.5:4b", Path::new(FIXTURE_PATH))
        .await
        .unwrap_or_else(|err| panic!("triage --file deve funcionar offline: {err}"));
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.first(), Some(&"Causa raiz provável:"), "{output}");
    assert!(
        output.contains("\nTTL do token muito curto."),
        "texto do modelo deve aparecer dentro da seção: {output}"
    );
    assert!(lines.contains(&"Arquivos prováveis:"), "{output}");
    assert!(output.contains("\nsrc/auth/session.rs"), "{output}");
    assert!(lines.contains(&"Confiança:"), "{output}");
    assert!(output.contains("\nmédia"), "{output}");
    // The prompt sent to the (fake) model carries the fixture data, cleaned.
    let request = server
        .join()
        .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"));
    let Some(request) = request else {
        panic!("servidor falso deve capturar a requisição");
    };
    assert!(request.contains("POST /api/chat"), "{request}");
    assert!(
        request.contains("Sessão expirada com token válido"),
        "prompt deve conter o título da fixture: {request}"
    );
    assert!(
        !request.contains("<div>"),
        "prompt não pode conter HTML cru: {request}"
    );
}
