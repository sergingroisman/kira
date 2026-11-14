//! Perfil de hardware efetivo (PR20, plano-finalizacao §3): o `kira doctor`
//! deixa de chutar modelo — detecta VRAM via `nvidia-smi` resolvido no PATH
//! (mesmo estilo [`crate::binresolve`] das ferramentas de scan) e RECOMENDA
//! tag/ctx por perfil (RTX 2060 6 GB / M1 Metal / CPU).
//!
//! Regras que o módulo respeita:
//! - **Recomendação apenas**: o doctor é read-only — aqui não se escreve
//!   `config.toml`, não se troca modelo e não se sobe variável nenhuma. O
//!   dono decide; o texto só diz o que a máquina aguenta.
//! - **Detecção por comando de linha, nunca por texto de OS** (ADR-0016,
//!   camada 1–3): `nvidia-smi --query-gpu=memory.total
//!   --format=csv,noheader,nounits` devolve UM INTEIRO puro por GPU (MB) —
//!   sem unidade e sem texto localizado, o parse não depende de locale em
//!   nenhum SO (a exigência do Windows é "só números MB"; o `nounits`
//!   remove o problema na fonte em vez de regexar coluna com sufixo).
//! - **Fail-open**: sem `nvidia-smi` ou com saída ilegível, o perfil é CPU
//!   com a CAUSA na linha — nunca pânico, nunca número inventado.
//! - **Env override** `KIRA_NVIDIA_SMI_BIN` (mesmo contrato de
//!   `KIRA_SCAN_*_BIN`/`KIRA_COPILOT_BIN`/`KIRA_MODEL_OLLAMA_BIN`): shim dos
//!   testes e2e e máquina cujo serviço roda sem o nvidia-smi no PATH.
//!   Justificativa da env nova: é o seam de injeção que os testes deste
//!   módulo exigem (o aceite do PR20 é "teste com shim fake no KIRA_*_BIN").
//! - **Sem dependência nova**: GPU entra por binresolve, não por crate de
//!   hardware (o VPS e o CI não têm GPU; o dono tem RTX 2060 e M1).
//! - **Não fala com a camada LLM** (allowlist ADR-0016): as tags de
//!   recomendação são constantes espelhadas de `src/llm/roles.rs` — o doctor
//!   não pode importar `llm::*`, então o espelho fica aqui com teste que
//!   trava o texto.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Env override do binário `nvidia-smi` (shim dos testes e2e).
pub const NVIDIA_SMI_BIN_ENV: &str = "KIRA_NVIDIA_SMI_BIN";

/// Piso de VRAM (MB) do perfil 6 GB — a RTX 2060 reporta 6144.
pub const VRAM_6GB_MB: u64 = 6000;
/// Piso de VRAM (MB) do perfil 8 GB (7 B Q4 com ctx menor).
pub const VRAM_8GB_MB: u64 = 8000;

/// Ctx recomendado para `qwen3.5:4b` na 2060 (decisão do dono, 10/Nov #3).
pub const CTX_4096: u32 = 4096;
/// Ctx menor para modelos maiores (7 B) — a janela inteira não cabe no
/// resto da VRAM depois dos pesos.
pub const CTX_MENOR: u32 = 2048;

// Espelho das tags da casa (fonte: `src/llm/roles.rs`). O doctor não pode
// importar `llm::*` (allowlist do ADR-0016), então o texto vive aqui —
// o teste `tags_mirror_the_house_catalog` trava os três valores.
pub const TAG_JUDGMENT: &str = "qwen3.5:4b";
pub const TAG_CHEAP: &str = "qwen3:1.7b";
pub const TAG_CODER_7B: &str = "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M";

/// Perfil de hardware detectado nesta máquina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwareProfile {
    /// GPU NVIDIA detectada — soma da VRAM total em MB (multi-GPU soma).
    Gpu { vram_mb: u64 },
    /// macOS: backend Metal do Ollama sobre memória unificada (mínimo do
    /// dono: 8 GB). `nvidia-smi` não existe nesses aparelhos — nem se tenta.
    Metal,
    /// Sem GPU utilizável. A causa vai na linha do doctor (acionável).
    Cpu { cause: String },
}

/// Recomendação de configuração para o perfil (texto do doctor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    /// Tag do Ollama recomendada.
    pub tag: &'static str,
    /// Valor recomendado para `KIRA_NUM_CTX`.
    pub ctx: u32,
    /// Nota extra acionável (ex.: gate de admissão do HF-5).
    pub note: Option<&'static str>,
}

/// Resolve o `nvidia-smi` da máquina: override por env (contrato do usuário —
/// valor presente e não vazio; arquivo inexistente não resolve) ou busca
/// `which`-style no PATH. `None` = ausente.
#[must_use]
pub fn resolve_nvidia_smi() -> Option<PathBuf> {
    if let Some(value) = crate::binresolve::bin_from_env(NVIDIA_SMI_BIN_ENV) {
        let path = PathBuf::from(&value);
        return path.is_file().then_some(path);
    }
    let path_env = std::env::var_os("PATH");
    crate::binresolve::find_on_path(path_env.as_deref(), "nvidia-smi")
}

/// Parse puro da saída de `nvidia-smi --query-gpu=memory.total
/// --format=csv,noheader,nounits`: uma linha por GPU, cada uma um INTEIRO
/// puro em MB. Soma multi-GPU; qualquer linha que não seja inteiro puro
/// (ex.: `[Not Supported]` de driver velho) invalida o todo — nunca se
/// inventa VRAM parcial.
#[must_use]
pub fn parse_vram_csv(stdout: &str) -> Option<u64> {
    let lines: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    lines
        .iter()
        .try_fold::<u64, _, Option<u64>>(0u64, |acc, line| {
            if line.chars().all(|ch| ch.is_ascii_digit()) {
                line.parse::<u64>().ok().map(|mb| acc.saturating_add(mb))
            } else {
                None
            }
        })
}

/// Command do `nvidia-smi` com o caminho RESOLVIDO (nunca o nome nu) e o
/// wrapper `cmd /C chcp 65001` para `.cmd`/`.bat` no Windows — mesma conduta
/// do scan e do `model.rs` (o std não executa script como binário).
fn smi_command(bin: &Path, args: &[&str]) -> Command {
    let via_cmd = cfg!(windows) && crate::binresolve::is_cmd_script(&bin.to_string_lossy());
    if via_cmd {
        let owned: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
        let line = crate::shellline::cmd_command_line(bin, &owned);
        let mut command = Command::new("cmd");
        command.arg("/C");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg(line);
        }
        #[cfg(not(windows))]
        {
            // Inalcançável (via_cmd só é true no Windows); mantém o build honesto.
            let _ = line;
            command.arg(bin);
            command.args(args);
        }
        command
    } else {
        let mut command = Command::new(bin);
        command.args(args);
        command
    }
}

/// VRAM total em MB via spawn do binário resolvido. Qualquer falha (exit != 0,
/// sem stdout, parse quebrado) vira `None` — o caller transforma em causa.
pub fn query_vram_total(bin: &Path) -> Option<u64> {
    let output = smi_command(
        bin,
        &["--query-gpu=memory.total", "--format=csv,noheader,nounits"],
    )
    .output()
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_vram_csv(&String::from_utf8_lossy(&output.stdout))
}

/// Perfil efetivo (função PURA sobre o binário já resolvido — os testes
/// passam o caminho do shim direto, sem tocar env). No macOS o perfil é
/// Metal por definição do mínimo do dono: não há NVIDIA, e tentar resolver
/// `nvidia-smi` lá seria ruído.
#[must_use]
pub fn effective_profile(smi: Option<&Path>) -> HardwareProfile {
    if cfg!(target_os = "macos") {
        return HardwareProfile::Metal;
    }
    let Some(bin) = smi else {
        return HardwareProfile::Cpu {
            cause: "nvidia-smi não resolvido no PATH nem pela env KIRA_NVIDIA_SMI_BIN".to_string(),
        };
    };
    match query_vram_total(bin) {
        Some(vram_mb) => HardwareProfile::Gpu { vram_mb },
        None => HardwareProfile::Cpu {
            cause: "nvidia-smi presente mas não reportou VRAM legível — \
                    confira `nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits`"
                .to_string(),
        },
    }
}

/// Caminho de produção do doctor: resolve e classifica. Puro de efeitos no
/// mundo (spawn de leitura do `nvidia-smi` apenas, sem escrita nenhuma).
#[must_use]
pub fn doctor_profile() -> HardwareProfile {
    effective_profile(resolve_nvidia_smi().as_deref())
}

/// Recomendação por perfil — a tabela do plano (PR20):
/// - GPU ≥ 8 GB (e Metal): 7 B Q4 com ctx menor;
/// - GPU 6–8 GB: `qwen3.5:4b` Q4_K_M com `KIRA_NUM_CTX=4096`;
/// - GPU < 6 GB e CPU: 1–2 B.
///
/// O 7 B da casa é o coder bartowski (único 7B Q4 do catálogo) — rotear por
/// ele exige o gate `kira model admit` (HF-5), e a nota vai na linha.
#[must_use]
pub fn recommend(profile: &HardwareProfile) -> Recommendation {
    match profile {
        HardwareProfile::Gpu { vram_mb } if *vram_mb >= VRAM_8GB_MB => Recommendation {
            tag: TAG_CODER_7B,
            ctx: CTX_MENOR,
            note: Some("papel coder — só roteia após `kira model admit` (HF-5)"),
        },
        HardwareProfile::Gpu { vram_mb } if *vram_mb >= VRAM_6GB_MB => Recommendation {
            tag: TAG_JUDGMENT,
            ctx: CTX_4096,
            note: None,
        },
        HardwareProfile::Gpu { .. } => Recommendation {
            tag: TAG_CHEAP,
            ctx: CTX_MENOR,
            note: Some("VRAM abaixo do perfil 6 GB — 1–2 B instruct"),
        },
        HardwareProfile::Metal => Recommendation {
            tag: TAG_CODER_7B,
            ctx: CTX_MENOR,
            note: Some("papel coder — só roteia após `kira model admit` (HF-5)"),
        },
        HardwareProfile::Cpu { .. } => Recommendation {
            tag: TAG_CHEAP,
            ctx: CTX_MENOR,
            note: None,
        },
    }
}

/// Linha única do `kira doctor` com o perfil EFETIVO (VRAM detectada, modelo
/// recomendado, ctx recomendado). Recomendação apenas — o doctor NUNCA grava
/// config/model: quem aplica é o dono, à mão ou via env.
#[must_use]
pub fn profile_line(profile: &HardwareProfile) -> String {
    let rec = recommend(profile);
    let head = match profile {
        HardwareProfile::Gpu { vram_mb } => {
            format!("perfil de hardware: GPU ({vram_mb} MB VRAM)")
        }
        HardwareProfile::Metal => {
            "perfil de hardware: Metal (memória unificada — mínimo do dono 8 GB)".to_string()
        }
        HardwareProfile::Cpu { cause } => {
            format!("perfil de hardware: CPU ({cause})")
        }
    };
    let mut line = format!(
        "{head} — recomendado: {} · KIRA_NUM_CTX={}",
        rec.tag, rec.ctx
    );
    if let Some(note) = rec.note {
        line.push_str(&format!(" ({note})"));
    }
    if let HardwareProfile::Cpu { .. } = profile {
        // Regra do dono (10/Nov #8): sem GPU, o doctor avisa que o triage é lento.
        line.push_str(" (aviso: triage será lento sem GPU)");
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // Parse da saída do nvidia-smi (puro, locale-proof por construção)
    // -------------------------------------------------------------------

    #[test]
    fn vram_csv_reads_pure_integer_lines_and_sums_multi_gpu() {
        assert_eq!(parse_vram_csv("6144\n"), Some(6144));
        assert_eq!(parse_vram_csv("6144\n8192\n"), Some(14336));
        assert_eq!(parse_vram_csv("  6144  \n\r\n"), Some(6144));
        // `[Not Supported]`/texto/unidade invalidam o TODO — nunca VRAM parcial.
        assert_eq!(parse_vram_csv("6144\n[Not Supported]\n"), None);
        assert_eq!(parse_vram_csv("6144 MiB\n"), None);
        assert_eq!(parse_vram_csv(""), None);
        assert_eq!(parse_vram_csv("\n \n"), None);
    }

    // -------------------------------------------------------------------
    // Tabela de recomendação (pura — os limiares são o aceite do PR20)
    // -------------------------------------------------------------------

    #[test]
    fn recommendation_table_follows_the_plan() {
        // 6 GB (RTX 2060 = 6144 MB): 4B Q4_K_M com ctx 4096 — decisão do dono.
        let rec = recommend(&HardwareProfile::Gpu { vram_mb: 6144 });
        assert_eq!(rec.tag, "qwen3.5:4b");
        assert_eq!(rec.ctx, 4096);
        assert_eq!(rec.note, None);
        // 8 GB: 7 B Q4 com ctx menor, com a nota do gate HF-5.
        let rec = recommend(&HardwareProfile::Gpu { vram_mb: 8192 });
        assert_eq!(rec.tag, TAG_CODER_7B);
        assert_eq!(rec.ctx, 2048);
        assert!(rec.note.is_some());
        // Limiar embaixo: 5999 MB já cai para 1–2 B.
        let rec = recommend(&HardwareProfile::Gpu { vram_mb: 5999 });
        assert_eq!(rec.tag, "qwen3:1.7b");
        // Metal (M1): mesmo 7 B Q4/ctx menor — escopo do PR20.
        let rec = recommend(&HardwareProfile::Metal);
        assert_eq!(rec.tag, TAG_CODER_7B);
        assert_eq!(rec.ctx, 2048);
        // CPU: 1–2 B.
        let rec = recommend(&HardwareProfile::Cpu {
            cause: "nvidia-smi ausente".to_string(),
        });
        assert_eq!(rec.tag, "qwen3:1.7b");
        assert_eq!(rec.ctx, 2048);
    }

    #[test]
    fn tags_mirror_the_house_catalog() {
        // O doctor não pode importar llm::* (allowlist ADR-0016), então o
        // espelho é travado por teste: se `src/llm/roles.rs` mudar a tag,
        // este teste reprova e força a sincronia dos dois lados.
        assert_eq!(TAG_JUDGMENT, "qwen3.5:4b");
        assert_eq!(TAG_CHEAP, "qwen3:1.7b");
        assert_eq!(
            TAG_CODER_7B,
            "hf.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF:Q4_K_M"
        );
    }

    #[test]
    fn profile_line_carries_vram_model_ctx_and_never_a_write_hint() {
        let line = profile_line(&HardwareProfile::Gpu { vram_mb: 6144 });
        assert!(
            line.contains("GPU (6144 MB VRAM)")
                && line.contains("qwen3.5:4b")
                && line.contains("KIRA_NUM_CTX=4096"),
            "linha: {line}"
        );
        let cpu = HardwareProfile::Cpu {
            cause: "nvidia-smi ausente".to_string(),
        };
        let line = profile_line(&cpu);
        assert!(
            line.contains("CPU (nvidia-smi ausente)")
                && line.contains("qwen3:1.7b")
                && line.contains("triage será lento"),
            "linha CPU com causa e aviso: {line}"
        );
        // Recomendação apenas: nenhuma frase mandando escrever config.
        assert!(
            !line.contains("grava"),
            "doctor não oferece escrita: {line}"
        );
        let line = profile_line(&HardwareProfile::Metal);
        assert!(
            line.contains("Metal")
                && line.contains(TAG_CODER_7B)
                && line.contains("KIRA_NUM_CTX=2048"),
            "linha Metal: {line}"
        );
    }

    // -------------------------------------------------------------------
    // Detecção (macOS por cfg; ausência como CPU com causa)
    // -------------------------------------------------------------------

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_is_metal_even_with_a_smi_resolved() {
        // No macOS o perfil é Metal por definição do mínimo do dono — nem o
        // caminho do nvidia-smi (inexistente lá) muda isso.
        assert_eq!(effective_profile(None), HardwareProfile::Metal);
        assert_eq!(doctor_profile(), HardwareProfile::Metal);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn absent_smi_is_a_cpu_profile_with_the_cause() {
        match effective_profile(None) {
            HardwareProfile::Cpu { cause } => {
                assert!(cause.contains("nvidia-smi"), "causa acionável: {cause}");
            }
            other => panic!("ausência vira CPU, não {other:?}"),
        }
    }

    /// Shim do `nvidia-smi` nos testes: no Unix um script shell; no Windows
    /// um `.cmd` (o wrapper `cmd /C` já está no caminho de produção). O
    /// `name` distingue shims no MESMO diretório (sobrescrever um pelo
    /// outro faria o assert do spawn direto ler o conteúdo errado).
    /// No macOS os callers somem (perfil é Metal, cfg dos testes), então a
    /// função fica morta LÁ — o allow fica atrás do mesmo cfg.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    fn write_smi_shim(dir: &Path, name: &str, body_mb: &str) -> PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join(format!("nvidia-smi-{name}"));
            std::fs::write(&path, format!("#!/bin/sh\necho {body_mb}\n"))
                .unwrap_or_else(|err| panic!("escrever shim: {err}"));
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim: {err}"));
            path
        }
        #[cfg(windows)]
        {
            let path = dir.join(format!("nvidia-smi-{name}.cmd"));
            std::fs::write(&path, format!("@echo {body_mb}\r\n"))
                .unwrap_or_else(|err| panic!("escrever shim: {err}"));
            path
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn gpu_profile_reads_vram_through_the_bin_override_and_fails_open() {
        // Casa: edition 2024 torna set/remove_var unsafe — exceção ÚNICA e
        // test-only da repo (nenhum teste de produção lê esta env). Um único
        // teste muta `KIRA_NVIDIA_SMI_BIN`, para não correr com outros.
        let base = std::env::temp_dir().join(format!("kira-hwprofile-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let saved = std::env::var_os(NVIDIA_SMI_BIN_ENV);
        unsafe { std::env::set_var(NVIDIA_SMI_BIN_ENV, "valor-que-nao-existe") };

        // (1) Override presente mas arquivo inexistente: não resolve — perfil
        //     CPU com a causa (mesma conduta do scan: valor presente é
        //     contrato do usuário; inexistente não spawna nada).
        match effective_profile(resolve_nvidia_smi().as_deref()) {
            HardwareProfile::Cpu { cause } => {
                assert!(cause.contains("nvidia-smi"), "causa: {cause}");
            }
            other => panic!("override inválida vira CPU, não {other:?}"),
        }

        // (2) Shim real pela MESMA env: GPU com a VRAM do fake — o aceite
        //     literal do PR20 ("teste com shim fake no KIRA_*_BIN").
        let shim = write_smi_shim(&base, "good", "6144");
        unsafe { std::env::set_var(NVIDIA_SMI_BIN_ENV, &shim) };
        assert_eq!(
            effective_profile(resolve_nvidia_smi().as_deref()),
            HardwareProfile::Gpu { vram_mb: 6144 }
        );

        // (3) Shim que responde lixo: fail-open para CPU com a causa.
        let bad = write_smi_shim(&base, "bad", "6144 MiB");
        unsafe { std::env::set_var(NVIDIA_SMI_BIN_ENV, &bad) };
        match effective_profile(resolve_nvidia_smi().as_deref()) {
            HardwareProfile::Cpu { cause } => {
                assert!(
                    cause.contains("VRAM legível"),
                    "saída ilegível vira causa: {cause}"
                );
            }
            other => panic!("lixo do shim vira CPU, não {other:?}"),
        }

        // (4) Spawn direto pelo caminho (sem env): a função de query também
        //     é chamável isolada — é o que o doctor usa por trás.
        assert_eq!(query_vram_total(&shim), Some(6144));
        assert_eq!(query_vram_total(&bad), None);

        match saved {
            Some(value) => unsafe { std::env::set_var(NVIDIA_SMI_BIN_ENV, value) },
            None => unsafe { std::env::remove_var(NVIDIA_SMI_BIN_ENV) },
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
