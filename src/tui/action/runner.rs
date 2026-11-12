//! Runner da ação: executa o plano em thread e devolve eventos por canal.
//!
//! Por que thread + canal: `scan` e `triage` levam segundos (LLM pode passar
//! de 30 s) e o loop da TUI não pode bloquear — um `wait()` no meio do desenho
//! congela a tela. Aqui o loop só drena o canal a cada frame.
//!
//! Garantias do ADR-0033:
//! - `stdin(null)`: o filho nunca espera input que o terminal não vai dar;
//! - stdout/stderr em pipe, lidos por DUAS threads (pipe cheio não trava);
//! - nada de shell: o programa é o `current_exe` que veio no plano;
//! - `Drop` mata o filho: sair do dashboard não deixa processo órfão.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::tui::action::log::{push_line, push_note};
use crate::tui::action::{ActionPlan, ExecState, ExecStatus, Stream};

/// Intervalo de checagem do processo (e da flag de cancelamento).
const PUMP_INTERVAL_MS: u64 = 30;

/// Um evento da execução, do runner para a TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// Processo criado (o overlay já pode dizer "executando").
    Started { command: String },
    /// Uma linha BRUTA: quem sanitiza e redige é o [`ExecState::apply`].
    Line { stream: Stream, text: String },
    /// Nota do próprio KIRA (teto de buffer, cancelamento).
    Note(String),
    /// O processo saiu (`code` = `None` quando morto por sinal).
    Exited { code: Option<i32>, duration_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    Spawn(String),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::Spawn(cause) => write!(
                formatter,
                "não foi possível executar a ação: {cause} — o CLI continua disponível no terminal"
            ),
        }
    }
}

/// Execução viva: canal de eventos + alça para matar o filho.
pub struct Runner {
    pub rx: Receiver<RunEvent>,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Runner {
    /// Spawna o plano e começa a bombear eventos.
    pub fn spawn(plan: ActionPlan) -> Result<Self, RunnerError> {
        let mut command = Command::new(&plan.program);
        command
            .args(&plan.argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|err| RunnerError::Spawn(err.to_string()))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(RunEvent::Started {
            command: plan.command_line(),
        });

        let cancel = Arc::new(AtomicBool::new(false));
        let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
        let thread_cancel = Arc::clone(&cancel);
        let thread_child = Arc::clone(&child_slot);
        let started = Instant::now();

        let handle = std::thread::spawn(move || {
            let mut readers = Vec::new();
            if let Some(stream) = stdout {
                let stream_tx = tx.clone();
                readers.push(std::thread::spawn(move || {
                    read_stream(stream, Stream::Out, stream_tx)
                }));
            }
            if let Some(stream) = stderr {
                let stream_tx = tx.clone();
                readers.push(std::thread::spawn(move || {
                    read_stream(stream, Stream::Err, stream_tx)
                }));
            }

            // `try_wait` em vez de `wait`: mantém o mutex livre para o
            // `cancel()` (que precisa de `&mut Child` para matar).
            let mut code = None;
            loop {
                if thread_cancel.load(Ordering::SeqCst)
                    && let Ok(mut slot) = thread_child.lock()
                    && let Some(child) = slot.as_mut()
                {
                    let _ = child.kill();
                }
                let finished = match thread_child.lock() {
                    Ok(mut slot) => match slot.as_mut() {
                        Some(child) => match child.try_wait() {
                            Ok(Some(status)) => {
                                code = status.code();
                                slot.take();
                                true
                            }
                            Ok(None) => false,
                            Err(_) => {
                                slot.take();
                                true
                            }
                        },
                        None => true,
                    },
                    Err(_) => true,
                };
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(PUMP_INTERVAL_MS));
            }

            // Toda linha já está no canal quando o `Exited` chega: sem isso o
            // overlay diria "concluído" com metade da saída faltando.
            for reader in readers {
                let _ = reader.join();
            }
            let _ = tx.send(RunEvent::Exited {
                code,
                duration_ms: duration_ms(started.elapsed()),
            });
        });

        Ok(Self {
            rx,
            cancel,
            child: child_slot,
            handle: Some(handle),
        })
    }

    /// Pede o encerramento do filho (usado ao sair do dashboard).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.child.lock()
            && let Some(child) = slot.as_mut()
        {
            let _ = child.kill();
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Sair da TUI não pode deixar `scan`/`triage` rodando escondido.
        self.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl ExecState {
    /// Aplica um evento do runner: sanitiza, redige e mantém o teto.
    pub fn apply(&mut self, event: RunEvent) {
        match event {
            RunEvent::Started { .. } => {}
            RunEvent::Line { stream, text } => {
                push_line(&mut self.lines, &mut self.dropped, stream, &text);
            }
            RunEvent::Note(text) => push_note(&mut self.lines, &mut self.dropped, &text),
            RunEvent::Exited { code, duration_ms } => {
                self.status = ExecStatus::Exited { code };
                let label = match code {
                    Some(0) => "concluído (exit 0)".to_string(),
                    Some(code) => format!("falhou (exit {code})"),
                    None => "encerrado sem código de saída".to_string(),
                };
                push_note(
                    &mut self.lines,
                    &mut self.dropped,
                    &format!(
                        "{label} · {}",
                        crate::tui::state::format_duration_ms(duration_ms as i64)
                    ),
                );
            }
        }
    }
}

/// Lê uma linha por vez e manda crua; erro/EOF encerra a thread.
fn read_stream(stream: impl Read + Send + 'static, kind: Stream, tx: mpsc::Sender<RunEvent>) {
    let mut reader = BufReader::new(stream);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&strip_line_end(&buffer)).to_string();
                if tx.send(RunEvent::Line { stream: kind, text }).is_err() {
                    break; // receiver caiu (app fechou): para de ler
                }
            }
            Err(_) => break,
        }
    }
}

/// Tira só o `\n`/`\r` de fim de linha (o `\r` do MEIO é progresso).
fn strip_line_end(buffer: &[u8]) -> Vec<u8> {
    let mut bytes = buffer.to_vec();
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    bytes
}

/// `Duration` em ms com saturação (u64 cobre o horizonte de um comando).
fn duration_ms(elapsed: Duration) -> u64 {
    elapsed.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_stream_sends_lines_and_stops_at_eof() {
        let (tx, rx) = mpsc::channel();
        read_stream(&b"a\r\nb\n"[..], Stream::Out, tx);
        let events: Vec<RunEvent> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                RunEvent::Line {
                    stream: Stream::Out,
                    text: "a".to_string()
                },
                RunEvent::Line {
                    stream: Stream::Out,
                    text: "b".to_string()
                },
            ]
        );
    }

    #[test]
    fn strip_line_end_leaves_the_progress_carriage_return() {
        assert_eq!(strip_line_end(b"100%\r\n"), b"100%".to_vec());
        // O `\r` do MEIO é progresso: quem resolve é o sanitizador da tela.
        assert_eq!(strip_line_end(b"0%\r50%\n"), b"0%\r50%".to_vec());
    }

    #[test]
    fn apply_sanitizes_redacts_and_closes_with_a_note() {
        let mut exec = ExecState::new(crate::tui::action::Action::Scan, 0);
        exec.apply(RunEvent::Line {
            stream: Stream::Err,
            text: "\u{1b}[31mfalhou\u{1b}[0m".to_string(),
        });
        exec.apply(RunEvent::Exited {
            code: Some(7),
            duration_ms: 1_200,
        });
        assert_eq!(exec.lines.len(), 2);
        assert_eq!(exec.lines[0].text(), "falhou");
        assert!(exec.lines[0].text().contains("falhou"));
        assert_eq!(exec.status, ExecStatus::Exited { code: Some(7) });
        assert!(
            exec.lines[1].text().contains("exit 7"),
            "{:?}",
            exec.lines[1]
        );
        assert!(!exec.is_running());
    }

    /// O runner real, com o PRÓPRIO binário de teste como filho: `--list`
    /// escrito em stdout e saída 0. Nada de shell no caminho.
    #[test]
    fn runner_streams_stdout_and_reports_the_exit_code() {
        let program = std::env::current_exe().unwrap_or_else(|err| panic!("exe: {err}"));
        let plan = ActionPlan {
            action: crate::tui::action::Action::Scan,
            program,
            argv: vec!["--list".to_string()],
            root: std::env::temp_dir(),
            effects: Vec::new(),
            prerequisites: Vec::new(),
        };
        let runner = Runner::spawn(plan).unwrap_or_else(|err| panic!("spawn: {err}"));
        let mut seen_stdout = false;
        let mut exited = None;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let Ok(event) = runner.rx.recv_timeout(Duration::from_millis(500)) else {
                continue;
            };
            match event {
                RunEvent::Line {
                    stream: Stream::Out,
                    ..
                } => seen_stdout = true,
                RunEvent::Exited { code, .. } => {
                    exited = Some(code);
                    break;
                }
                RunEvent::Started { .. } | RunEvent::Line { .. } | RunEvent::Note(_) => {}
            }
        }
        assert!(seen_stdout, "stdout do filho chegou linha a linha");
        assert_eq!(exited, Some(Some(0)), "exit code propagado");
    }
}
