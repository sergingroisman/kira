//! Offline tests of the live `inspect`/`triage` HTTP error mapping.
//!
//! No real Azure DevOps: a std `TcpListener` serves canned error responses
//! (`401`, `404` with `TF401232`, `500`) on 127.0.0.1, exactly like the fake
//! ADO server in `tests/setup_e2e.rs`. The PAT in the tests is fake.
//!
//! Expected messages:
//! - `401` → points at the PAT token page (`/_usersSettings/tokens`);
//! - `404` with `TF401232` → the three actionable hypotheses, with the id, and
//!   WITHOUT the raw response body;
//! - `500` → compact `HTTP 500` message, no body;
//! - `404` without the TF code → still compact (`AdoStatus`), no body;
//! - success output is unchanged (covered by `setup_e2e`).

use std::io::{Read, Write};
use std::net::TcpListener;

use kira::board::BoardProvider;
use kira::board::azure::AdoBoard;
use kira::error::Error;

/// Exact body the real Azure DevOps returned for the repro (bug of 523373).
const TF401232_BODY: &str = r#"{"$id":"1","innerException":null,"message":"TF401232: Work item 523373 does not exist, or you do not have permissions to read it.","typeName":"Microsoft.TeamFoundation.WorkItemTracking.Server.WorkItemUnauthorizedAccessException, Microsoft.TeamFoundation.WorkItemTracking.Server","typeKey":null,"errorCode":0,"eventId":3200}""#;

/// Fake Azure DevOps: answers every request with `status_line` + `body`.
///
/// Same pattern as `tests/setup_e2e.rs::spawn_fake_ado`, but keeps accepting
/// connections in a loop so an eventual SDK retry still gets an answer.
fn spawn_fake_ado(
    status_line: &'static str,
    body: &'static str,
) -> (String, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    let handle = std::thread::spawn(move || {
        let mut first_request: Option<String> = None;
        while let Ok((mut stream, _)) = listener.accept() {
            let request = read_http_request(&mut stream);
            if first_request.is_none() {
                first_request = Some(request);
            }
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if stream.write_all(response.as_bytes()).is_err() {
                continue;
            }
            let _ = stream.flush();
        }
        first_request
    });
    (format!("http://{addr}"), handle)
}

/// Reads until the full HTTP request (headers + content-length body) is in.
/// Same logic as `tests/setup_e2e.rs` — a GET without `content-length` uses
/// zero, never a guessed length (a wrong fallback makes the server wait for
/// bytes that never come and the client times out).
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

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn fake_board(endpoint: &str) -> AdoBoard {
    AdoBoard::with_endpoint(endpoint, "PAT-FALSO-DE-TESTE", "minha-org", "Kira")
}

#[tokio::test]
async fn get_work_item_maps_401_to_the_pat_page() {
    let (endpoint, _server) = spawn_fake_ado(
        "HTTP/1.1 401 Unauthorized",
        r#"{"$id":"1","message":"TF400813: The user not authorized."}"#,
    );
    let Err(err) = fake_board(&endpoint).work_item(523373).await else {
        panic!("401 deve virar erro no caminho wit");
    };
    let message = err.to_string();
    assert!(
        message.contains("PAT inválido"),
        "erro deve ser acionável: {message}"
    );
    assert!(
        message.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
        "erro deve apontar a página do token: {message}"
    );
}

#[tokio::test]
async fn get_work_item_maps_tf401232_to_the_three_hypotheses() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 404 Not Found", TF401232_BODY);
    let Err(err) = fake_board(&endpoint).work_item(523373).await else {
        panic!("404 com TF401232 deve virar erro no caminho wit");
    };
    let message = err.to_string();
    assert!(
        message.contains("#523373"),
        "erro deve citar o id: {message}"
    );
    assert!(
        message.contains("(1) o PAT não tem o escopo Work Items (Read)"),
        "hipótese 1 do PAT: {message}"
    );
    assert!(
        message.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
        "hipótese 1 aponta a página do token: {message}"
    );
    assert!(
        message.contains("(2) o item pertence a outro projeto — rode com --project"),
        "hipótese 2 do projeto: {message}"
    );
    assert!(
        message.contains("https://dev.azure.com/minha-org/Kira/_workitems/edit/523373"),
        "hipótese 2 aponta a URL do item no browser: {message}"
    );
    assert!(
        message.contains("(3) o id está errado"),
        "hipótese 3 do id: {message}"
    );
    assert!(
        !message.contains(TF401232_BODY),
        "o corpo cru da resposta nunca aparece no erro: {message}"
    );
    assert!(
        !message.contains("innerException"),
        "nenhuma parte do JSON cru pode vazar: {message}"
    );
    assert!(
        !message.contains("\"$id\""),
        "nenhuma parte do JSON cru pode vazar: {message}"
    );
}

#[tokio::test]
async fn get_work_item_maps_500_to_a_compact_error_without_body() {
    let (endpoint, _server) = spawn_fake_ado(
        "HTTP/1.1 500 Internal Server Error",
        r#"{"impossivel":"erro interno com muitas chaves e detalhes"}"#,
    );
    let Err(err) = fake_board(&endpoint).work_item(4821).await else {
        panic!("500 deve virar erro no caminho wit");
    };
    let message = err.to_string();
    assert!(
        message.contains("HTTP 500"),
        "erro deve citar o status: {message}"
    );
    assert!(
        message.contains("https://dev.azure.com/minha-org"),
        "erro deve apontar a organização: {message}"
    );
    assert!(
        !message.contains("\"impossivel\""),
        "o corpo cru da resposta nunca aparece no erro: {message}"
    );
}

#[tokio::test]
async fn get_work_item_maps_404_without_tf_code_to_a_compact_error() {
    let (endpoint, _server) = spawn_fake_ado(
        "HTTP/1.1 404 Not Found",
        r#"{"$id":"1","message":"o runtime não achou nada sobre isso"}"#,
    );
    let Err(err) = fake_board(&endpoint).work_item(999999).await else {
        panic!("404 sem TF-código deve virar erro");
    };
    match err {
        Error::AdoStatus {
            status,
            organization,
        } => {
            assert_eq!(status, 404);
            assert_eq!(organization, "minha-org");
        }
        other => panic!("404 sem TF-código deve virar AdoStatus, veio: {other}"),
    }
}
