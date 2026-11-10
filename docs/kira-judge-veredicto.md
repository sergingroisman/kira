# Veredicto da juíza independente — Mandato KIRA v2 (base do mandato v3)

> Juíza: `claude-haiku-5-5` via OpenCode · 10/Nov/2026 · Fonte: prompt pirâmide
> `planos/kira-judge-mandato.md` contendo as 7 intenções verbatim do dono.
> Resultado: **BLOQUEADO · fidelidade à intenção: 5/10** — os bloqueios abaixo
> viram patches no mandato v3.

## 1. VEREDITO
`BLOQUEADO · 5/10` — perdeu-se a intenção 2 (Devin + Delta com o dono no circuito
de aprovação); o gate do M2 não tinha método de medição; blog era inflação.

## 2. Intenções (síntese do mapa da juíza)
| # | Status | Nota |
|---|---|---|
| 1 autonomia | deferida-legítimo | reconciliação loop-vs-daemon escrita correta; só começa pós-gate |
| 2 Devin+Delta | **perdida** → corrigida | gera `awaiting_approval` + `kira approve <id>` (mandato v3) |
| 3 Datadog | atendida | setup pergunta a fonte |
| 4 IA local + Copilot | atendida | "cliente instala o Ollama" explícito |
| 5 instalador | atendida | install.ps1 obrigatório |
| 6 commands.toml | perdida-parcial → corrigida | `toml` vira dep justificada + golden por OS |
| 7 provas da main | atendida | com lacuna do gate (bloqueio) |

## 3. Inflações/omissões corrigidas no v3
- Budget sem número → `max_llm_calls_per_day = 50` default em config.toml
- "costly" sem dono → quem marca: dono/operador via `kira note`
- BYOK descartado sem registro → PR11b no ROADMAP
- PII "ROADMAP" vs logs-no-prompt (contradição) → PII vira pré-requisito do PR12
- Agendamento no PR14 antes do modo existir (PR16) → movido ao PR16
- "PR inócuo em loop" sem teste → teste de 2 ciclos idênticos = critério do PR15
- Blog no PR18 → removido do mandato; ROADMAP (a confirmar)

## 4. Os 3 PRs mais frágeis (com destravamento)
| PR | Risco | Destrava |
|---|---|---|
| PR11 Copilot SDK | pin preview instável; assinatura/policy da Avanade; se IT bloquear, plano sem saída | PR só com policy confirmada por escrito; ollama-only = caminho válido, não falha; BYOK = PR11b |
| PR12 Datadog | 2 chaves (API+APP) + site regional; logs estouram KIRA_NUM_CTX=4096; PII | teto de linhas/tokens na janela; PII como pré-requisito; chaves no keyring |
| PR14 instalador | agendamento antes do modo; ExecutionPolicy; cadeia de suprimento (rustup/ollama sem checksum) | agendamento → PR16; `Set-ExecutionPolicy -Scope Process`; validar presença e orientar |

## 5. O que o dono precisa providenciar antes do gate M2
(a) policy Copilot CLI na Avanade; (b) application key Datadog de leitura + site
regional; (c) orçamento real de chamadas Copilot (número); (d) **20 bugs fechados
com causa raiz verdadeira rotulada por ele**; (e) decisão do ExecutionPolicy.
Sem (d), o M2 nunca se mede — e a autonomia fica bloqueada para sempre.
