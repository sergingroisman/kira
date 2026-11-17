//! `kira daemon install|uninstall` (DAEMON-02) — coloca o daemon para acordar
//! sozinho no login/boot: pasta Inicializar (Startup) do usuário no Windows,
//! launchd no macOS (plist `RunAtLoad` em `~/Library/LaunchAgents`). Sem
//! dependência nova e sem serviço nosso: a entrada é só um convite para o SO
//! spawnar o MESMO binário (`kira daemon run --path <raiz>`) — o lock por
//! projeto (DAEMON-01) e a cadência da tabela `schedule` continuam sendo a
//! fonte de verdade.
//!
//! Contratos que o módulo prova:
//!
//! 1. **Nome determinístico por projeto.** `kira-<nome>-<8 hex>` — a mesma
//!    raiz gera sempre o mesmo atalho; instalar de novo SOBRESCREVE, nunca
//!    duplica (a reescrita do .cmd e do plist é idempotente).
//! 2. **O filho é o próprio binário atual** (`current_exe`), com `--path`
//!    absoluto — nada de shell na linha e NENHUM segredo nela (o daemon
//!    não precisa de PAT: o ciclo é local).
//! 3. **Windows sem spawn** (ADR-0045): install/uninstall são pura escrita/
//!    remoção de arquivo na pasta Inicializar — o EDR do hardware do dono
//!    bloqueia `schtasks /Create` vindo de pai não-aprovado (provado
//!    empiricamente, inclusive em terminal admin), e enrolar schtasks em
//!    batch para escapar da inspeção é tática de malware. No macOS o
//!    `launchctl` segue resolvido no PATH antes do spawn (override
//!    `KIRA_LAUNCHCTL_BIN` para os e2e); ausência NUNCA spawna.
//! 4. **Nunca parse de texto localizado do OS**: a decisão usa exit code; a
//!    dica de erro é só a primeira linha de stderr, compacta (`binresolve`).
//!    A validação real (login do dono, launchd do M1) é o bloqueio B5 do
//!    plano — aqui a prova é o conteúdo dos arquivos (funções puras).

use std::path::Path;
// `PathBuf` só aparece nos caminhos com agendador nativo — no Linux o import
// viria morto e `-D warnings` reprova o gate (prova: CI ubuntu).
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::path::PathBuf;
// O `Command` só existe onde há ferramenta do SO para spawnar (launchctl) —
// no Windows o install é escrita de arquivo, sem spawn.
#[cfg(target_os = "macos")]
use std::process::Command;

use crate::store;

/// Override do binário `launchctl` (macOS) — e2e aponta um shim.
pub const LAUNCHCTL_BIN_ENV: &str = "KIRA_LAUNCHCTL_BIN";

/// Erros do install/uninstall — todos acionáveis (a casa nunca ecoa body cru).
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error(
        "não consegui localizar o próprio binário do KIRA (current_exe falhou) — rode o kira por um caminho estável (instalado, não `cargo run`)"
    )]
    CurrentExe,
    #[error("raiz ilegível: {0}")]
    Root(String),
    #[error(
        "daemon install não está implementado neste SO — o MVP cobre Windows (schtasks) e macOS (launchd); rode `kira daemon run` em primeiro plano"
    )]
    UnsupportedOs,
    #[error(
        "{program} não foi encontrado — confira a instalação do SO ou aponte a env de override"
    )]
    ToolMissing { program: &'static str },
    #[error("não foi possível executar o {program}: {message}")]
    Spawn {
        program: &'static str,
        message: String,
    },
    #[error("a ferramenta {program} falhou (exit {code}) — confira o que o agendador disse{hint}")]
    ToolFailed {
        program: &'static str,
        code: i32,
        hint: String,
    },
    #[error("não consegui escrever {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

/// O que um install fez — a linha humana sai do handler do CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    /// Nome determinístico da tarefa (`kira-<nome>-<8 hex>`).
    pub task: String,
    /// Detalhe honesto do que ficou registrado no agendador.
    pub detail: String,
}

/// Nome determinístico da tarefa do daemon para ESTA raiz:
/// `kira-<nome saneado>-<primeiros 8 hex do project_id>`. A mesma pasta
/// (canônica) gera sempre o mesmo nome — instalar de novo sobrescreve.
pub fn task_name(root: &Path) -> Result<String, InstallError> {
    let project_id =
        store::project_id_for_root(root).map_err(|err| InstallError::Root(err.to_string()))?;
    let hash8: String = project_id.chars().take(8).collect();
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let raw = canonical.file_name().map_or_else(
        || "proj".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    // Saneamento: o Task Scheduler aceita quase tudo, mas o nome vira arquivo
    // no launchd e aparece em `schtasks /Query` — ASCII simples é o seguro.
    let slug: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "proj" } else { slug };
    let short: String = slug.chars().take(32).collect();
    Ok(format!("kira-{short}-{hash8}"))
}

/// `kira daemon install`: registra a tarefa que acorda o daemon no boot/login.
///
/// O KIRA não vira serviço: a tarefa do SO é o convite — quem garante "um
/// daemon por projeto" continua sendo o lock (DAEMON-01) e a cadência
/// continua sendo a tabela `schedule`.
pub fn install(root: &Path) -> Result<InstallReport, InstallError> {
    #[cfg(target_os = "windows")]
    {
        install_windows(root)
    }
    #[cfg(target_os = "macos")]
    {
        install_macos(root)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = root;
        Err(InstallError::UnsupportedOs)
    }
}

/// `kira daemon uninstall`: remove a tarefa. Idempotente no coração: tarefa
/// que não está lá vira "nada a remover" (o plist é a fonte de verdade no
/// launchd; no schtasks o query decide por EXIT CODE, nunca por texto).
pub fn uninstall(root: &Path) -> Result<String, InstallError> {
    #[cfg(target_os = "windows")]
    {
        uninstall_windows(root)
    }
    #[cfg(target_os = "macos")]
    {
        uninstall_macos(root)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = root;
        Err(InstallError::UnsupportedOs)
    }
}

// ---------------------------------------------------------------------------
// Windows — pasta Inicializar (Startup) do usuário
//
// POR QUE NÃO `schtasks` (ADR-0045): o EDR do hardware do dono bloqueia no
// CreateProcess o `schtasks /Create` vindo de pai não-aprovado — provado
// empiricamente em 17/Nov/2026 (probe→`/?` ok, probe→`/Create` negado,
// kira→`/Create` negado, linha visível de schtasks dentro de `cmd` negada) —
// e a validaçăo em terminal ADMIN falhou do mesmo jeito. Enrolar o schtasks
// dentro de batch para escapar da inspeção de linha é tática de malware —
// o KIRA não faz isso. A pasta Inicializar é o equivalente em espaço de
// usuário do gatilho ONLOGON: acorda no login, sem privilégio, sem spawn
// (install/uninstall são pura escrita/remoção de arquivo), visível e
// removível pelo dono. Sem watchdog: se o processo morrer, volta no próximo
// login — o `kira daemon status` mostra o lock órfão e `--force` recupera.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn install_windows(root: &Path) -> Result<InstallReport, InstallError> {
    let task = task_name(root)?;
    let (exe, root_display) = exe_and_root_display(root)?;
    let dir = startup_dir()?;
    std::fs::create_dir_all(&dir).map_err(|source| InstallError::Write {
        path: dir.display().to_string(),
        source,
    })?;
    let shortcut = dir.join(format!("{task}.cmd"));
    let content = startup_cmd_content(&task, &exe, &root_display);
    std::fs::write(&shortcut, content).map_err(|source| InstallError::Write {
        path: shortcut.display().to_string(),
        source,
    })?;
    Ok(InstallReport {
        task: task.clone(),
        detail: format!(
            "atalho gravado na pasta Inicializar (roda no próximo login, sobrescreve): {} — \"{}\" daemon run --path \"{}\"",
            shortcut.display(),
            exe,
            root_display
        ),
    })
}

#[cfg(target_os = "windows")]
fn uninstall_windows(root: &Path) -> Result<String, InstallError> {
    let task = task_name(root)?;
    let dir = startup_dir()?;
    let shortcut = dir.join(format!("{task}.cmd"));
    match std::fs::remove_file(&shortcut) {
        Ok(()) => Ok(format!(
            "atalho {task} removido da pasta Inicializar ({})",
            shortcut.display()
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(format!(
            "nada a remover — o atalho {} não existe (nunca instalado, ou já removido)",
            shortcut.display()
        )),
        Err(source) => Err(InstallError::Write {
            path: shortcut.display().to_string(),
            source,
        }),
    }
}

/// Pasta Inicializar do USUÁRIO (nunca a comum de todos os usuários — o
/// install não pede elevação): `%APPDATA%\Microsoft\Windows\Start Menu\
/// Programs\Startup`. O override de env (`APPDATA`) é o seam dos e2e.
#[cfg(target_os = "windows")]
fn startup_dir() -> Result<PathBuf, InstallError> {
    let appdata = std::env::var_os("APPDATA")
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            InstallError::Root(
                "sem %APPDATA% — não há onde registrar o start automático".to_string(),
            )
        })?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup"))
}

/// Conteúdo do atalho: puro e determinístico (mesma raiz ⇒ mesmo arquivo).
/// `start "título" /MIN` abre o console MINIMIZADO — honestidade visual:
/// o dono VÊ que o KIRA está vivo (persona §7: nada de atividade invisível).
/// Um daemon já vivo é recusado pelo lock (DAEMON-01) e o console fecha.
#[cfg(target_os = "windows")]
fn startup_cmd_content(task: &str, exe: &str, root: &str) -> String {
    // Comentário ASCII de propósito: batch é lido no code page OEM do console,
    // e bytes UTF-8 fora de ASCII viram mojibake lá (a linha `start` já é
    // ASCII pura — caminhos e argumentos não podem ser manglados).
    format!(
        "@echo off\r\n\
         rem KIRA - daemon do projeto {root}\r\n\
         rem gravado por `kira daemon install`; remover com `kira daemon uninstall`\r\n\
         start \"kira daemon {task}\" /MIN \"{exe}\" daemon run --path \"{root}\"\r\n"
    )
}

// ---------------------------------------------------------------------------
// macOS — launchd (plist em ~/Library/LaunchAgents)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn install_macos(root: &Path) -> Result<InstallReport, InstallError> {
    let task = task_name(root)?;
    let (exe, root_display) = exe_and_root_display(root)?;
    let dir = launch_agents_dir()?;
    std::fs::create_dir_all(&dir).map_err(|source| InstallError::Write {
        path: dir.display().to_string(),
        source,
    })?;
    let plist = dir.join(format!("{task}.plist"));
    let content = plist_content(&task, &exe, &root_display);
    std::fs::write(&plist, content).map_err(|source| InstallError::Write {
        path: plist.display().to_string(),
        source,
    })?;
    let launchctl = resolve_platform_tool("launchctl", LAUNCHCTL_BIN_ENV)?;
    let args = ["load".to_string(), plist.display().to_string()];
    let result = run_tool_code(&launchctl, &args);
    if result.code != 0 {
        // O plist FICOU gravado: o dono decide (repetir o install, ou
        // `kira daemon uninstall`) — mentir "instalado" não é opção.
        return Err(InstallError::ToolFailed {
            program: "launchctl",
            code: result.code,
            hint: format!(
                " (o plist {} ficou gravado; desfaça com `kira daemon uninstall`{})",
                plist.display(),
                result.hint_suffix()
            ),
        });
    }
    Ok(InstallReport {
        task,
        detail: format!(
            "plist gravado e carregado (RunAtLoad): {} — \"{}\" daemon run --path \"{}\"",
            plist.display(),
            exe,
            root_display
        ),
    })
}

#[cfg(target_os = "macos")]
fn uninstall_macos(root: &Path) -> Result<String, InstallError> {
    let task = task_name(root)?;
    let dir = launch_agents_dir()?;
    let plist = dir.join(format!("{task}.plist"));
    if !plist.exists() {
        return Ok(format!(
            "nada a remover — o plist {} não existe (nunca instalado, ou já removido)",
            plist.display()
        ));
    }
    let launchctl = resolve_platform_tool("launchctl", LAUNCHCTL_BIN_ENV)?;
    let args = ["unload".to_string(), plist.display().to_string()];
    let result = run_tool_code(&launchctl, &args);
    if result.code != 0 {
        return Err(InstallError::ToolFailed {
            program: "launchctl",
            code: result.code,
            hint: format!(
                " (o plist {} ficou gravado; remova à mão se quiser forçar{})",
                plist.display(),
                result.hint_suffix()
            ),
        });
    }
    std::fs::remove_file(&plist).map_err(|source| InstallError::Write {
        path: plist.display().to_string(),
        source,
    })?;
    Ok(format!(
        "tarefa {task} descarregada e plist removido ({})",
        plist.display()
    ))
}

// ---------------------------------------------------------------------------
// Peças compartilhadas — só onde há o que spawnar (launchctl no macOS). O
// Windows do install é escrita de arquivo; no Linux os caminhos que as usam
// nem existem, e `dead_code` sob `-D warnings` reprova o gate (CI ubuntu).
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "windows", target_os = "macos"))]
/// Binário atual + raiz canônica para display (a tarefa roda em cwd do SO —
/// os dois caminhos precisam ser absolutos e sem o prefixo `\\?\`).
fn exe_and_root_display(root: &Path) -> Result<(String, String), InstallError> {
    let exe = std::env::current_exe()
        .map_err(|_| InstallError::CurrentExe)?
        .canonicalize()
        .map_err(|_| InstallError::CurrentExe)?;
    let canonical = root.canonicalize().map_err(|err| {
        InstallError::Root(format!(
            "a raiz do projeto não foi encontrada ({err}) — rode dentro do repositório"
        ))
    })?;
    Ok((store::display_path(&exe), store::display_path(&canonical)))
}

#[cfg(target_os = "macos")]
fn launch_agents_dir() -> Result<PathBuf, InstallError> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()))
        .ok_or_else(|| {
            InstallError::Root("sem diretório home — não há onde gravar o plist".to_string())
        })?;
    Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
}

/// Resolve a ferramenta do ANTES do spawn (mesmo contrato do scan): override
/// de env é contrato do usuário (arquivo inexistente = ausente), depois PATH.
/// Só o macOS usa (o launchctl); o Windows do install é escrita de arquivo.
#[cfg(target_os = "macos")]
fn resolve_platform_tool(
    program: &'static str,
    override_env: &str,
) -> Result<PathBuf, InstallError> {
    if let Some(bin) = crate::binresolve::bin_from_env(override_env) {
        let path = PathBuf::from(bin);
        if path.is_file() {
            return Ok(path);
        }
        return Err(InstallError::ToolMissing { program });
    }
    let path_env = std::env::var_os("PATH");
    crate::binresolve::find_on_path(path_env.as_deref(), program)
        .ok_or(InstallError::ToolMissing { program })
}

/// Uma linha capturada da ferramenta: exit code + dica compacta.
#[cfg(target_os = "macos")]
struct Outcome {
    code: i32,
    hint: String,
}

#[cfg(target_os = "macos")]
impl Outcome {
    /// ` (dica: …)` quando há primeira linha de stderr; vazio quando não.
    fn hint_suffix(&self) -> String {
        if self.hint.is_empty() {
            String::new()
        } else {
            format!(" (dica: {})", self.hint)
        }
    }
}

/// Spawna a ferramenta resolvida (caminho COMPLETO, nunca o nome nu) e
/// captura exit + dica. `.cmd`/`.bat` no Windows vão pelo wrapper `cmd /C`
/// (mesma conduta do scan — o shim do e2e é um script).
#[cfg(target_os = "macos")]
fn run_tool_code(bin: &Path, args: &[String]) -> Outcome {
    let mut command = command_for(bin, args);
    match command.output() {
        Ok(output) => Outcome {
            code: output.status.code().unwrap_or(-1),
            hint: crate::binresolve::first_stderr_line(&String::from_utf8_lossy(&output.stderr)),
        },
        Err(source) => {
            // Spawn que nem começou: a dica é o motivo compacto do próprio SO.
            let mut hint = crate::binresolve::first_stderr_line(&source.to_string());
            if hint.is_empty() {
                hint = source.to_string().chars().take(200).collect();
            }
            Outcome { code: -1, hint }
        }
    }
}

/// Command sobre um binário resolvido: `.cmd`/`.bat` no Windows via
/// `cmd /C chcp 65001 …` (o std não executa script; o chcp mantém UTF-8) —
/// espelho do `command_from_resolved` do scan, usando a MESMA `shellline`.
/// Inalcançável no macOS (binários nunca são .cmd), mantido para o build
/// honesto e as provas do caminho de spawn.
#[cfg(target_os = "macos")]
fn command_for(bin: &Path, args: &[String]) -> Command {
    let via_cmd = cfg!(windows) && crate::binresolve::is_cmd_script(&bin.to_string_lossy());
    if via_cmd {
        let owned: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
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

/// Plist do launchd: conteúdo determinístico (mesma raiz ⇒ mesmo arquivo).
/// XML escapado nos caminhos — espaço é comum, `<`/`&` não são, mas escapar
/// é barato e o arquivo é fonte de verdade do uninstall.
#[cfg(target_os = "macos")]
fn plist_content(label: &str, exe: &str, root: &str) -> String {
    let xml = |text: &str| -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \x20 <key>Label</key>\n\
         \x20 <string>{}</string>\n\
         \x20 <key>ProgramArguments</key>\n\
         \x20 <array>\n\
         \x20   <string>{}</string>\n\
         \x20   <string>daemon</string>\n\
         \x20   <string>run</string>\n\
         \x20   <string>--path</string>\n\
         \x20   <string>{}</string>\n\
         \x20 </array>\n\
         \x20 <key>RunAtLoad</key>\n\
         \x20 <true/>\n\
         </dict>\n\
         </plist>\n",
        xml(label),
        xml(exe),
        xml(root)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // Os testes rodam em TODOS os alvos; o import do topo é gated por
    // agendador nativo, então o módulo de testes traz o próprio.
    use std::path::PathBuf;

    fn sandbox(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kira-daemon-install-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn task_name_is_deterministic_and_sanitized() {
        let root = sandbox("nome");
        let first = task_name(&root).expect("nome da tarefa");
        let second = task_name(&root).expect("nome da tarefa de novo");
        assert_eq!(first, second, "a mesma raiz gera o mesmo nome");
        // Formato: kira-<slug>-<8 hex> — slug ASCII e hash FNV truncado.
        let body = first.strip_prefix("kira-").expect("prefixo kira-");
        let (slug, hash) = body.rsplit_once('-').expect("nome-hash no fim");
        assert!(!slug.is_empty(), "{first}");
        assert!(
            slug.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "slug saneado: {first}"
        );
        assert_eq!(hash.len(), 8, "{first}");
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()), "{first}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn task_name_survives_weird_directory_names() {
        let root = sandbox("Espaço Estranho!!!");
        let name = task_name(&root).expect("nome da tarefa");
        assert!(name.starts_with("kira-"), "{name}");
        // Espaço e exclamação viram hífen; o nome inteiro cabe em [a-z0-9-].
        let slug: String = name.chars().skip(5).take_while(|ch| *ch != '-').collect();
        assert!(
            slug.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-'),
            "{name}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_content_has_label_program_and_path() {
        let content = plist_content("kira-demo-1234abcd", "/usr/bin/kira", "/Users/d/proj");
        assert!(
            content.contains("<string>kira-demo-1234abcd</string>"),
            "{content}"
        );
        assert!(
            content.contains("<string>/usr/bin/kira</string>"),
            "{content}"
        );
        assert!(
            content.contains("<string>/Users/d/proj</string>"),
            "{content}"
        );
        assert!(content.contains("<string>daemon</string>"), "{content}");
        assert!(content.contains("RunAtLoad"), "{content}");
        assert!(content.starts_with("<?xml version=\"1.0\""), "{content}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_content_escapes_xml_specials() {
        let content = plist_content("kira-x", "/a&b/kira", "/c<d>");
        assert!(content.contains("a&amp;b"), "{content}");
        assert!(content.contains("c&lt;d&gt;"), "{content}");
    }

    // O conteúdo REAL do atalho do Windows (sem shim no meio): o filho é o
    // próprio binário com --path absoluto, console minimizado e título.
    #[cfg(target_os = "windows")]
    #[test]
    fn startup_cmd_content_has_the_exact_command() {
        let content = startup_cmd_content(
            "kira-demo-1234abcd",
            r"C:\tools\kira.exe",
            r"C:\Users\d\Meu Projeto\repo",
        );
        assert!(
            content.contains(
                r#"start "kira daemon kira-demo-1234abcd" /MIN "C:\tools\kira.exe" daemon run --path "C:\Users\d\Meu Projeto\repo""#
            ),
            "{content}"
        );
        // Sem credencial no arquivo (o daemon é local).
        assert!(
            !content.to_lowercase().contains("azure_devops_pat")
                && !content.to_lowercase().contains("token="),
            "{content}"
        );
        // CRLF de script Windows + instrução de remoção para o dono.
        assert!(content.contains("\r\n"), "{content}");
        assert!(content.contains("kira daemon uninstall"), "{content}");
    }
}
