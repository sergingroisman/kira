//! Classificação do endpoint do Ollama (OC-1 — Ollama Cloud direto).
//!
//! Módulo PURO e FORA da camada `llm` de propósito: o `doctor` precisa das
//! mesmas respostas (URL local? tag `*:cloud`?) sem importar `llm::*` — a
//! allowlist do ADR-0016 reprova `doctor` falando com a camada de backend.
//! Aqui não há rede, não há modelo: só casamento de texto determinístico.
//!
//! Regra do OC-1: URL não-local É egresso — o prompt sai da máquina (o cloud
//! processa; não treina com ele, porém ele SAI). Aviso, nunca erro silencioso.

/// Env que aponta o servidor Ollama (sempre existiu; a constante mora aqui
/// para `doctor` e `llm` lerem o MESMO nome de um único dono).
pub const OLLAMA_URL_ENV: &str = "KIRA_OLLAMA_URL";

/// Default: servidor local na porta canônica do Ollama.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Lê a URL base do Ollama de `KIRA_OLLAMA_URL`, caindo no default local.
/// Em branco conta como ausente (typo na env não pode apontar para "").
#[must_use]
pub fn base_url_from_env() -> String {
    env_or_default(OLLAMA_URL_ENV, DEFAULT_BASE_URL)
}

fn env_or_default(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// True quando o request NUNCA sai da máquina: `localhost`, `127.x.x.x` ou
/// `::1` (IPv6 entre colchetes, com ou sem porta). Qualquer outro host é
/// não-local — inclusive URL sem esquema, porque chute é proibido aqui:
/// não-parseável = não-local = exigir chave é o lado seguro.
#[must_use]
pub fn is_local_url(base_url: &str) -> bool {
    let rest = match base_url.find("://") {
        Some(pos) => &base_url[pos + 3..],
        None => base_url,
    };
    // Porta e caminho fora: host é o que vem antes do primeiro `/` (e a porta
    // antes do primeiro `:` — exceto IPv6, que vem entre colchetes).
    let host = match rest.find('/') {
        Some(pos) => &rest[..pos],
        None => rest,
    };
    let host = match host.strip_prefix('[') {
        Some(stripped) => match stripped.find(']') {
            Some(end) => &stripped[..end],
            None => stripped,
        },
        None => match host.find(':') {
            Some(pos) => &host[..pos],
            None => host,
        },
    };
    let host = host.trim().to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

/// True quando a tag aponta para um modelo servido PELO CLOUD do Ollama
/// (`*:cloud` no jargão do plano): termina em `:cloud` ou `-cloud`
/// (`qwen3.5:cloud`, `qwen3-coder:480b-cloud`). Case-insensitive de propósito:
/// tag de config escrita à mão não pode escapar do aviso por maiúscula.
#[must_use]
pub fn is_cloud_tag(tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    tag.ends_with(":cloud") || tag.ends_with("-cloud")
}

/// Motivo do egresso pela URL (aviso ≠ erro): `Some` quando o prompt SAI da
/// máquina. Mensagem pronta para o `digest` (stderr) e o `loop` (nota).
#[must_use]
pub fn egress_reason_for_url(url: &str) -> Option<String> {
    if is_local_url(url) {
        None
    } else {
        Some(format!(
            "URL {url} não é local — o prompt sai da máquina (o cloud processa; não treina com ele, porém ele sai)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_urls_are_the_localhost_family() {
        for url in [
            "http://localhost:11434",
            "http://localhost",
            "http://127.0.0.1:11434",
            "http://127.0.0.1",
            "https://127.9.9.9/api",
            "http://[::1]:11434",
            "http://[::1]",
            "http://LOCALHOST:11434", // case-insensitive: host de URL é
        ] {
            assert!(is_local_url(url), "{url} deveria ser local");
        }
    }

    #[test]
    fn anything_else_is_not_local() {
        for url in [
            "https://ollama.com",
            "https://ollama.com/api",
            "http://192.168.0.10:11434",
            "http://meu-servidor.local:11434",
            "ollama.com", // sem esquema: não-parseável = não-local (lado seguro)
            "",
        ] {
            assert!(!is_local_url(url), "{url:?} NÃO deveria ser local");
        }
    }

    #[test]
    fn cloud_tags_end_in_cloud_suffix() {
        assert!(is_cloud_tag("qwen3.5:cloud"));
        assert!(is_cloud_tag("qwen3-coder:480b-cloud"));
        assert!(is_cloud_tag("GPT-OSS:120B-CLOUD")); // config à mão não escapa
        assert!(is_cloud_tag(" fake:cloud "));
        assert!(!is_cloud_tag("qwen3.5:4b"));
        assert!(!is_cloud_tag("hf.co/bartowski/X:Q4_K_M"));
        assert!(!is_cloud_tag("cloudy")); // sufixo no fim, não no meio
    }

    #[test]
    fn egress_reason_only_for_non_local_urls() {
        assert!(egress_reason_for_url("http://localhost:11434").is_none());
        let reason = egress_reason_for_url("https://ollama.com").expect("não-local tem motivo");
        assert!(reason.contains("sai da máquina"), "motivo: {reason}");
        assert!(
            egress_reason_for_url("http://127.0.0.1:1").is_none(),
            "127.x é a própria máquina: sem egresso"
        );
    }
}
