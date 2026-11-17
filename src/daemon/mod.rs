//! `kira daemon` (DAEMON-01) — o turno infinito do binário: fica vivo e roda
//! os ticks AGENDADOS (`kira schedule`) até o dono mandar parar (Ctrl+C).
//!
//! Sem daemon: `kira loop --scheduled` roda um turno e sai. Com daemon, é
//! "rodar enquanto eu durmo" — sem serviço do Windows no MVP: o daemon é um
//! modo do binário, em primeiro plano, com lock em disco.
//!
//! Contratos que o módulo prova:
//!
//! 1. **Um daemon por projeto.** O lock é `.kira/daemon.lock` com PID e
//!    instante; o segundo daemon é RECUSADO com erro acionável (fail-closed).
//!    `--force` quebra o lock — consentimento explícito do dono para um
//!    processo que morreu. Arquivo de lock ilegível TAMBÉM bloqueia (nada de
//!    "não entendi, então pode").
//! 2. **A cadência é do agendamento, nunca do daemon.** O sono entre checagens
//!    é derivado da tabela `schedule` (`wait_hint`) com teto de cortesia
//!    (`--max-wait`): o daemon acorda no máximo a cada `max-wait` para
//!    reavaliar, mas só ticka o que VENCEU. Sem agendamento nenhum, o daemon
//!    recusa (nada a fazer ≠ agenda vazia fingindo trabalho).
//! 3. **Saída sempre no modo texto do ciclo.** O daemon imprime o mesmo
//!    relatório do `kira loop` (tick + notas) — a TUI é modo, não substituto.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub mod install;

use crate::r#loop::{self, LoopOptions, TickReport};
use crate::scheduler::{self, SchedulePlan};
use crate::store::{self, ScheduleRow, Store};

/// Nome do arquivo de lock dentro de `.kira/`.
pub const LOCK_FILE: &str = "daemon.lock";

/// Erros do daemon — todos acionáveis (a casa nunca ecoa body cru).
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error(
        "já há um daemon neste projeto (lock {path}, {ident}) — pare o processo ou rode `kira daemon --force` se for um lock órfão"
    )]
    LockHeld { path: String, ident: String },
    #[error(
        "sem agendamento declarado — o daemon não tem cadência para seguir; crie com `kira schedule add --action digest --every 2h` (e veja `kira schedule list`)"
    )]
    NoSchedule,
    #[error("o ciclo do daemon falhou: {0}")]
    Cycle(String),
}

/// O que o lock diz sobre o dono dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockInfo {
    /// PID do processo que gravou (`None` = arquivo ilegível/escrito à mão).
    pub pid: Option<u32>,
    /// RFC 3339 UTC do instante em que o lock nasceu.
    pub since: Option<String>,
}

impl LockInfo {
    /// Identidade legível para o erro acionável.
    #[must_use]
    pub fn ident(&self) -> String {
        match (&self.pid, &self.since) {
            (Some(pid), Some(since)) => format!("PID {pid}, desde {since}"),
            _ => "arquivo de lock ilegível".to_string(),
        }
    }
}

/// Caminho do lock deste projeto.
#[must_use]
pub fn lock_path(root: &Path) -> PathBuf {
    root.join(".kira").join(LOCK_FILE)
}

/// Lê o lock existente. Ilegível NÃO vira `None` — vira `LockInfo` sem campos
/// (o chamador bloqueia do mesmo jeito; "não entendi" nunca autoriza).
#[must_use]
pub fn read_lock(root: &Path) -> LockInfo {
    let Ok(content) = std::fs::read_to_string(lock_path(root)) else {
        return LockInfo {
            pid: None,
            since: None,
        };
    };
    if content.trim().is_empty() {
        return LockInfo {
            pid: None,
            since: None,
        };
    }
    let mut info = LockInfo {
        pid: None,
        since: None,
    };
    for line in content.lines() {
        if let Some(pid) = line.strip_prefix("pid=") {
            info.pid = pid.trim().parse().ok();
        } else if let Some(since) = line.strip_prefix("since=") {
            info.since = Some(since.trim().to_string());
        }
    }
    info
}

/// Adquire o lock do projeto: `create_new` é atômico no FS, então dois
/// daemons simultâneos não podem ambos vencer. `--force` apaga o lock antigo
/// ANTES — consentimento explícito para um processo que morreu.
pub fn acquire_lock(root: &Path, force: bool) -> Result<LockInfo, DaemonError> {
    let path = lock_path(root);
    if force {
        let _ = std::fs::remove_file(&path);
    } else if path.exists() {
        let held = read_lock(root);
        return Err(DaemonError::LockHeld {
            path: path.display().to_string(),
            ident: held.ident(),
        });
    }
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
    let info = LockInfo {
        pid: Some(std::process::id()),
        since: Some(store::now_rfc3339_utc()),
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| {
            // A corrida com outro daemon cai aqui (create_new falha se já
            // existe): mesmo erro acionável do caminho "já existia".
            let held = read_lock(root);
            DaemonError::LockHeld {
                path: path.display().to_string(),
                ident: if err.kind() == std::io::ErrorKind::AlreadyExists {
                    held.ident()
                } else {
                    format!("lock não pôde ser criado ({err})")
                },
            }
        })?;
    write!(
        file,
        "pid={}\nsince={}\n",
        info.pid.unwrap_or(0),
        info.since.as_deref().unwrap_or("")
    )
    .and_then(|()| file.flush())
    .map_err(|err| DaemonError::Cycle(format!("lock não pôde ser escrito ({err})")))?;
    Ok(info)
}

/// Solta o lock. `true` quando existia (o status usa para dizer o que fez).
#[must_use]
pub fn release_lock(root: &Path) -> bool {
    std::fs::remove_file(lock_path(root)).is_ok()
}

/// Segundos até o PRÓXIMO vencimento (0 = algo vence agora) — a mesma conta
/// do [`scheduler::is_due`], derivada e nunca gravada. Linha com carimbo
/// ilegível não contribui (conservador, igual ao `is_due`): sem contribuição,
/// o daemon acorda no teto de cortesia e reavalia.
#[must_use]
pub fn wait_hint(rows: &[ScheduleRow], now_seconds: i64) -> u64 {
    let mut min_delta = u64::MAX;
    for row in rows {
        if !row.enabled {
            continue;
        }
        match row.last_run_at.as_deref() {
            // Nunca rodou ⇒ vencido agora (mesma decisão do `is_due`).
            None => return 0,
            // Carimbo ilegível: conservador (igual ao `is_due`) — a linha não
            // contribui e o daemon acorda no teto de cortesia para reavaliar.
            Some(last) => {
                let Some(last_seconds) = scheduler::parse_unix_seconds(last) else {
                    continue;
                };
                let delta = row.every_seconds - now_seconds.saturating_sub(last_seconds);
                if delta <= 0 {
                    return 0;
                }
                min_delta = min_delta.min(delta as u64);
            }
        }
    }
    // Sem linha válida nenhuma: não há cadência para derivar — devolve o
    // teto (o chamador faz `min(hint, max_wait)`). ZERO aqui seria loop
    // apertado: o tick não faria nada (is_due é conservador) e o daemon
    // re-tickaria sem parar.
    if min_delta == u64::MAX {
        u64::MAX
    } else {
        min_delta
    }
}

/// Opções de uma sessão de daemon.
#[derive(Debug, Clone)]
pub struct DaemonOptions {
    pub root: PathBuf,
    pub model: Option<String>,
    pub lang: Option<String>,
    pub skip_digest: bool,
    pub skip_scan: bool,
    /// Teto do sono entre checagens (cortesia; a cadência real é a tabela).
    pub max_wait_secs: u64,
    /// Teto de ticks por execução (CI/teste). `None` = até Ctrl+C.
    pub max_ticks: Option<usize>,
    /// Quebra um lock existente (`--force`).
    pub force_lock: bool,
}

impl DaemonOptions {
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            model: None,
            lang: None,
            skip_digest: false,
            skip_scan: false,
            max_wait_secs: 3_600,
            max_ticks: None,
            force_lock: false,
        }
    }
}

/// Uma linha de evento do daemon para o stdout (mesmo modo texto do ciclo).
fn tick_line(number: usize, report: &TickReport) -> String {
    format!("tick {number}: {}", report.line())
}

/// Roda o daemon: lock → aguardar vencimento → tick agendado → repetir até
/// Ctrl+C ou teto de ticks. Cada fase marcada no store manda no próximo sono;
/// um tick que não marca nada (falha transitória) ganha um descanso mínimo
/// para não virar loop apertado contra um provedor doente.
pub async fn run(options: &DaemonOptions) -> Result<String, DaemonError> {
    let lock = acquire_lock(&options.root, options.force_lock)?;
    let mut out = String::new();
    out.push_str(&format!(
        "daemon vivo: PID {} desde {} (lock {})\n",
        lock.pid.unwrap_or(0),
        lock.since.as_deref().unwrap_or("—"),
        lock_path(&options.root).display()
    ));

    // As MESMAS opções do `kira loop`, mas SEMPRE agendado: o daemon não é
    // cadência própria, é o executor da cadência do dono.
    let mut loop_options = LoopOptions::at(options.root.clone());
    if let Some(model) = &options.model {
        loop_options.model = model.clone();
    }
    match &options.lang {
        Some(lang) => loop_options.lang = lang.clone(),
        None => {
            if let Ok(lang) = std::env::var("KIRA_LANG") {
                loop_options.lang = lang;
            }
        }
    }
    loop_options.skip_digest = options.skip_digest;
    loop_options.run_scan = !options.skip_scan;
    loop_options.scheduled = true;

    // Repouso mínimo pós-tick: um tick que não marca nada (digest doente,
    // por exemplo) não pode re-tickar em rajada.
    let rest_after_tick = options.max_wait_secs.clamp(1, 60);
    let result = drive(options, &loop_options, rest_after_tick, &mut out).await;
    // O resultado é intencionalmente ignorado: o lock pode já ter sido
    // quebrado por `--force` em outra janela — soltar duas vezes é no-op.
    let _ = release_lock(&options.root);
    result.map(|()| out)
}

/// O corpo do daemon: aguarda vencimento → tick agendado → repete. Escreve a
/// saída em `out` (o modo texto do ciclo) e só erros de verdade sobem.
async fn drive(
    options: &DaemonOptions,
    loop_options: &LoopOptions,
    rest_after_tick: u64,
    out: &mut String,
) -> Result<(), DaemonError> {
    let mut ticks = 0usize;
    loop {
        let now = scheduler::parse_unix_seconds(&store::now_rfc3339_utc())
            .ok_or_else(|| DaemonError::Cycle("relógio do sistema ilegível".to_string()))?;
        let store = Store::open_default()
            .map_err(|err| DaemonError::Cycle(format!("store ilegível ({err})")))?;
        let plan =
            SchedulePlan::load(&store, now).map_err(|err| DaemonError::Cycle(err.to_string()))?;
        if plan.is_empty() {
            return Err(DaemonError::NoSchedule);
        }
        let rows = store
            .list_schedules()
            .map_err(|err| DaemonError::Cycle(err.to_string()))?;
        drop(store);
        let hint = wait_hint(&rows, now);
        let wait = hint.min(options.max_wait_secs).max(1);
        if hint > 0 {
            // Nada venceu: dorme até o próximo vencimento (ou o teto de
            // cortesia, ou o Ctrl+C — o que vier primeiro).
            if sleep_or_interrupt(wait).await {
                out.push_str("daemon parado com graça (Ctrl+C) — lock liberado\n");
                return Ok(());
            }
            continue;
        }
        ticks += 1;
        let report = r#loop::tick(loop_options)
            .await
            .map_err(|err| DaemonError::Cycle(err.to_string()))?;
        out.push_str(&tick_line(ticks, &report));
        out.push('\n');
        for note in &report.notes {
            out.push_str(&format!("  nota: {note}\n"));
        }
        if let Some(max) = options.max_ticks
            && ticks >= max
        {
            out.push_str(&format!(
                "teto de {max} tick(s) atingido — daemon sai (o lock foi liberado)\n"
            ));
            return Ok(());
        }
        if sleep_or_interrupt(rest_after_tick).await {
            out.push_str("daemon parado com graça (Ctrl+C) — lock liberado\n");
            return Ok(());
        }
    }
}

/// Dorme `secs` segundos; `true` quando o Ctrl+C chegou primeiro.
async fn sleep_or_interrupt(secs: u64) -> bool {
    tokio::select! {
        () = tokio::time::sleep(Duration::from_secs(secs)) => false,
        result = tokio::signal::ctrl_c() => {
            // Falha de handler de sinal (raro) não pode transformar o Ctrl+C
            // em infinite loop: segue dormindo o resto e o dono mata o PID.
            result.is_ok()
        }
    }
}

/// `kira daemon status`: o que o lock diz (sem checagem de vida de PID — isso
/// é trabalho de plataforma; o `--force` é a saída documentada para órfão).
#[must_use]
pub fn status(root: &Path) -> String {
    let path = lock_path(root);
    if !path.exists() {
        return format!(
            "nenhum daemon vivo (lock {} não existe) — rode `kira daemon`\n",
            path.display()
        );
    }
    let info = read_lock(root);
    format!(
        "daemon: lock {} ({})\nagendamentos: veja `kira schedule list` (a cadência é dele, não do daemon)\n",
        path.display(),
        info.ident()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kira-daemon-unit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn lock_is_created_refused_and_released() {
        let root = temp_root("lock");
        let first = acquire_lock(&root, false).expect("primeiro lock");
        assert_eq!(first.pid, Some(std::process::id()), "PID é o próprio");
        assert!(first.since.is_some(), "carimbo UTC");
        // O arquivo nasceu com o conteúdo esperado.
        let content = std::fs::read_to_string(lock_path(&root)).expect("lock legível");
        assert!(content.contains("pid="), "{content}");
        assert!(content.contains("since="), "{content}");
        // Segundo daemon é recusado com erro acionável.
        let err = acquire_lock(&root, false).expect_err("lock em uso");
        let message = err.to_string();
        assert!(message.contains("kira daemon --force"), "{message}");
        assert!(message.contains("pare o processo"), "{message}");
        // Lock ilegível (escrito à mão) TAMBÉM bloqueia, com identidade honesta.
        std::fs::write(lock_path(&root), "lixo sem formato").expect("escrever lixo");
        let err = acquire_lock(&root, false).expect_err("lock ilegível");
        assert!(
            err.to_string().contains("arquivo de lock ilegível"),
            "{}",
            err
        );
        // --force quebra o lock e a release devolve o que fez.
        let forced = acquire_lock(&root, true).expect("force quebra");
        assert_eq!(forced.pid, Some(std::process::id()));
        assert!(release_lock(&root), "lock existia");
        assert!(!release_lock(&root), "segunda release: nada a fazer");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wait_hint_is_zero_when_due_and_the_cadence_when_not() {
        let row = |every: i64, last: Option<&str>| ScheduleRow {
            id: 1,
            action: "digest".to_string(),
            every_seconds: every,
            enabled: true,
            last_run_at: last.map(String::from),
            created_at: "1970-01-01T00:00:00Z".to_string(),
        };
        // Nunca rodou: vencido agora (mesma decisão do `is_due`).
        assert_eq!(wait_hint(&[row(3_600, None)], 1_000), 0);
        // Rodou há 10s de uma cadência de 1h: faltam 3_590s.
        assert_eq!(
            wait_hint(&[row(3_600, Some("1970-01-01T00:00:09Z"))], 19),
            3_590
        );
        // Vencido (cadência estourada): 0.
        assert_eq!(
            wait_hint(&[row(3_600, Some("1970-01-01T00:00:00Z"))], 10_000),
            0
        );
        // O MENOR vencimento entre as linhas manda (falta 3000 e falta 20 → 20).
        let rows = [
            row(3_600, Some("1970-01-01T00:00:00Z")), // falta 3_000
            row(600, Some("1970-01-01T00:00:20Z")),   // elapsed 580 → falta 20
        ];
        assert_eq!(wait_hint(&rows, 600), 20);
        // Carimbo ilegível: conservador — não vence, não aperta o sono.
        assert_eq!(
            wait_hint(&[row(3_600, Some("carimbo ruim"))], 1_000),
            u64::MAX,
            "sem contribuição válida"
        );
    }

    #[test]
    fn status_is_honest_about_living_and_missing_daemons() {
        let root = temp_root("status");
        let absent = status(&root);
        assert!(absent.contains("nenhum daemon vivo"), "{absent}");
        assert!(absent.contains("kira daemon"), "{absent}");
        let _ = acquire_lock(&root, false).expect("lock");
        let alive = status(&root);
        assert!(alive.contains("daemon: lock"), "{alive}");
        assert!(
            alive.contains(&format!("PID {}", std::process::id())),
            "{alive}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
