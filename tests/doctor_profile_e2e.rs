//! End-to-end do perfil de hardware no `kira doctor` (PR20).
//!
//! O contrato no limite do CLI: o doctor IMPRIME o perfil efetivo (CPU/GPU/
//! Metal com a causa) e NUNCA escreve config — o config.toml do sandbox sai
//! byte-idêntico, e a VRAM detectada vem do `nvidia-smi` injetado pela env
//! `KIRA_NVIDIA_SMI_BIN` (shim, mesmo contrato do `KIRA_SCAN_*_BIN`).
//! Sem PAT/org/projeto o doctor para no passo 1: nunca toca rede.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-doctor-profile-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        Self { base, cwd, store }
    }

    fn config_path(&self) -> PathBuf {
        self.base.join("config.toml")
    }

    /// Grava um config.toml com conteúdo conhecido (prova de não-escrita).
    fn write_config(&self) {
        fs::write(self.config_path(), "dev_name = \"Tester\"\n")
            .unwrap_or_else(|err| panic!("escrever config: {err}"));
    }

    /// Shim do `nvidia-smi`: script shell no Unix, `.cmd` no Windows (o
    /// wrapper `cmd /C` da produção lida com isso).
    fn write_smi_shim(&self, body_mb: &str) -> PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = self.base.join("nvidia-smi");
            fs::write(&path, format!("#!/bin/sh\necho {body_mb}\n"))
                .unwrap_or_else(|err| panic!("escrever shim: {err}"));
            fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim: {err}"));
            path
        }
        #[cfg(windows)]
        {
            let path = self.base.join("nvidia-smi.cmd");
            fs::write(&path, format!("@echo {body_mb}\r\n"))
                .unwrap_or_else(|err| panic!("escrever shim: {err}"));
            path
        }
    }

    /// Roda o binário REAL `kira doctor` no sandbox. `PATH` vazio deixa a
    /// ausência de `nvidia-smi` determinística em qualquer runner (o binário
    /// é spawnado por caminho absoluto, então nem depende do PATH).
    fn run(&self, extra_envs: &[(&str, &str)], clear: &[&str]) -> (i32, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .arg("doctor")
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.config_path())
            .env("KIRA_SECRET_STORE", "off")
            .env("PATH", "")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_NVIDIA_SMI_BIN");
        for name in clear {
            command.env_remove(name);
        }
        for (name, value) in extra_envs {
            command.env(name, value);
        }
        let output = command
            .output()
            .unwrap_or_else(|err| panic!("executar kira doctor: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    fn config_bytes(&self) -> Vec<u8> {
        fs::read(self.config_path()).unwrap_or_else(|err| panic!("ler config: {err}"))
    }
}

/// O doctor sempre imprime a linha do perfil e NUNCA muda o config.toml.
/// Metal no macOS, CPU nos demais (com PATH vazio não há nvidia-smi).
#[test]
fn doctor_prints_the_profile_and_never_touches_config() {
    let sandbox = Sandbox::new("profile-line");
    sandbox.write_config();
    let before = sandbox.config_bytes();

    let (code, out) = sandbox.run(&[], &[]);
    assert!(
        out.contains("perfil de hardware"),
        "linha do perfil no doctor: {out}"
    );
    if cfg!(target_os = "macos") {
        assert!(out.contains("Metal"), "perfil macOS é Metal: {out}");
    } else {
        assert!(
            out.contains("CPU") && out.contains("nvidia-smi"),
            "sem nvidia-smi o perfil é CPU com a causa: {out}"
        );
        assert!(
            out.contains("qwen3:1.7b") && out.contains("KIRA_NUM_CTX=2048"),
            "CPU recomenda 1–2 B: {out}"
        );
    }
    // Doctor sem PAT é configuração incompleta — e mesmo assim imprimiu o perfil.
    assert_eq!(code, 1, "sem credenciais o doctor para no passo 1: {out}");
    // Prova de não-escrita: config.toml byte-idêntico depois do doctor.
    assert_eq!(
        sandbox.config_bytes(),
        before,
        "o doctor é read-only: config.toml não pode mudar"
    );
}

/// Com o shim pela env override, o doctor mostra o perfil GPU com a VRAM do
/// fake e a recomendação do perfil 6 GB (aceite (a) do PR20).
#[cfg(not(target_os = "macos"))]
#[test]
fn gpu_profile_comes_from_the_bin_override_shim() {
    let sandbox = Sandbox::new("gpu-shim");
    sandbox.write_config();
    let shim = sandbox.write_smi_shim("6144");
    let (code, out) = sandbox.run(
        &[("KIRA_NVIDIA_SMI_BIN", shim.display().to_string().as_str())],
        &[],
    );
    assert_eq!(code, 1, "doctor sem credenciais para no passo 1: {out}");
    assert!(
        out.contains("GPU (6144 MB VRAM)"),
        "VRAM do shim na linha: {out}"
    );
    assert!(
        out.contains("qwen3.5:4b") && out.contains("KIRA_NUM_CTX=4096"),
        "perfil 6 GB recomenda 4B/ctx 4096: {out}"
    );
    // Recomendação apenas: config.toml continua intacto depois do run.
    let config =
        fs::read_to_string(sandbox.config_path()).unwrap_or_else(|err| panic!("ler config: {err}"));
    assert_eq!(config, "dev_name = \"Tester\"\n");
}
