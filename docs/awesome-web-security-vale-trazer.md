# Relatório — O que vale trazer do awesome-web-security para o KIRA

- **Data**: 12/Nov/2026
- **Fonte**: `qazbnm456/awesome-web-security` — curadoria de material de web
  security (14k stars, licença **CC0**: domínio público; conceito e texto são
  reutilizáveis com atribuição). Análise sobre o README completo (2.004
  linhas), o `CLAUDE.md` do repo e o `skills/awesome-web-security/SKILL.md`
  (a lista também é servida como Skill para agentes de IA).
- **ADRs**: [ADR-0024](adr/ADR-0024-guardrails-invisiveis-prompt-v2.md) —
  guardrails de prompt/redação ·
  [ADR-0025](adr/ADR-0025-probe-suite-seguranca.md) — teste de segurança do
  próprio KIRA
- **Veredito**: 3 adaptações + 1 opcional **a confirmar com o dono**; 1 achado
  empírico novo (bypass do `redact` com caracteres invisíveis — probe de
  12/Nov/2026, abaixo).

## Por que uma lista de web-security interessa a um CLI read-only

O KIRA não é scanner de aplicação web — e a lista não entra por esse lado. O
valor está em três práticas que o repo externo executa e o KIRA pode copiar:

1. **Skill de IA com guardrails explícitos** (`SKILL.md`): como uma fonte
   curada é servida a um agente — quando usar, quando **recusar** (pedir
   escopo), formato fixo de saída e comportamento de falha ("do NOT fabricate
   entries").
2. **CI de segurança do próprio conteúdo** (`CLAUDE.md`): o repo roda um
   `diff-sentry` que **reprova payloads ofuscados, sinks de exfiltração,
   evasion por bidi/zero-width e misconfig de workflow** — exatamente a
   ameaça que um work item hostil representa para o prompt do `kira triage`.
3. **Ferramentas de leaking/deteção** (Tools > Leaking / Detecting):
   `gitleaks`, `retire.js`, `repo-supervisor` — o padrão do `kira scan`
   (resolve no PATH → parse → fingerprint) já é esse contrato de
   orquestração.

## O que vale trazer (concreto)

| # | Ideia da lista | Onde cai no KIRA | Prioridade |
|---|---|---|---|
| G1 | **Neutralização de caracteres invisíveis (zero-width/bidi) antes do `redact` e do pack** — U+00AD, U+200B–200D, U+2060–2064, U+FEFF, U+202A–202E, U+2066–2069 | `src/redact.rs` (ponto único — índice, pack, store, log e episódio herdam) | **Alta — bug real provado por probe** |
| G2 | **Prompt v2 com bloco delimitado de conteúdo não confiável + regra anti-instrução embutida** — estrutura de guardrails do `SKILL.md` (quando recusar, não fabricar, formato fixo) | `src/llm/prompts.rs` (`v1→v2`, fingerprint + fixtures no mesmo commit) | Alta |
| G3 | **Probe suite de segurança do próprio KIRA** — red team estático no `cargo test`, taxonomia OWASP LLM Top 10 | `tests/security_probe.rs` | Alta |
| G4 | **`gitleaks` como ferramenta do `kira scan`** (segredos no histórico do repo) | `src/scan/`, mais uma ferramenta orquestrada (KIRA orquestra, não analisa) | **Opcional — a confirmar com o dono** |

Fontes concretas: G1 — artigo "Be careful what you copy: Invisibly inserting
usernames into text with Zero-Width Characters" (seção Miscellaneous) + a
prática do `diff-sentry` no CI do próprio repo; G2 —
`skills/awesome-web-security/SKILL.md` (Safety guardrails / Failure handling);
G3 — `CLAUDE.md` (diff-sentry) + artigos "Where the LLM Stops: Deterministic
Scoring in an AI-Assisted VAPT Pipeline" e "The 200 That Proved Nothing"
(seção Practices > Application); G4 — Tools > Leaking (gitleaks).

## Achado empírico — probe de bypass do `redact` (12/Nov/2026)

Probe temporário (`tests/zz_probe_tmp.rs`, removido após a coleta) rodou 10
famílias de payload contra `kira::redact` — evidência capturada com
`cargo test --test zz_probe_tmp -- --nocapture`:

| Payload | Saída do `redact` | Veredito |
|---|---|---|
| `token=AbC\u{200B}123XyZ789QqQ` (ZWSP no valor) | `token=[REDACTED]` | mascarado |
| `token\u{200B}=AbC123XyZ789QqQ` (ZWSP entre chave e `=`) | **idêntico à entrada** | **vaza — zero máscara** |
| `let pat = "AbC\u{200D}123";` (ZWJ em valor) | `let pat = "[REDACTED]";` | mascarado |
| `ghp\u{200B}_0123456789abcdefghij` (ZWSP no shape) | **idêntico** | **vaza** |
| `Bearer\u{200B} eyJhbGciOiJIUzI1NiIsInR5cCI6` (ZWSP após Bearer) | **idêntico** | **vaza (JWT)** |
| `serjo\u{200B}ta@proton.me` (ZWSP no e-mail) | `serjo\u{200B}[EMAIL]` | **parcial** — prefixo local permanece |
| `doc 123\u{200B}.456.789-01` (ZWSP no CPF) | **idêntico** | **vaza** |
| `…?sig=abc\u{200B}DEF123456` (ZWSP em query) | `?sig=[REDACTED]` | mascarado |
| `token=A\u{2060}bC123XyZ789QqQ` (U+2060 no valor) | `token=[REDACTED]` | mascarado |
| `token\u{FEFF}=AbC123XyZ789QqQ` (U+FEFF entre chave e `=`) | **idêntico** | **vaza** |

**Balanço: 5 vazamentos completos + 1 parcial em 10 famílias.** Nos 5
completos, `contains_secret` também retorna `false` — falso negativo de
privacidade em toda a fronteira (log, store, pack, prompt). Causa técnica: no
`regex`, `\s` é `White_Space` e os Cf (U+200B/U+200C/U+200D/U+2060/U+FEFF)
**não são** `White_Space` — qualquer um deles entre a chave e o `=`, ou
dentro de um shape conhecido, quebra o match inteiro. É o mesmo vetor que o
`diff-sentry` do repo externo reprova em CI ("bidi/zero-width evasion"):
texto invisível para o humano, legível para o modelo — tanto como segredo
que escapa da máscara quanto como instrução escondida no work item.

## O que NÃO vale

- **Ferramentas ofensivas / pentest** (Burp, sqlmap, XSStrike, commix, ZAP,
  Nuclei, TIDoS): o KIRA mantém repositórios; não ataca aplicações vivas. Sem
  alvo, sem pentest — e sem instalar ferramenta sem checagem de procedência
  (limite inviolável nº 3 da persona).
- **OSINT / social engineering** (Shodan, FOCA, haveibeenpwned, Social
  Mapper): dado de cliente nunca sai da máquina; recon é fora de escopo.
- **Playbooks de ataque e payload lists como conteúdo**
  (PayloadsAllTheThings, FuzzDB): o KIRA não guarda payload alheio; a única
  lista de payloads do repo será a do probe suite — defensiva, em formato de
  teste.
- **Browser exploitation / WAF evasion / cloud pentest (AWS, Azure)**: sem
  superfície correspondente no KIRA.
- **`retire.js`**: overlap direto com `pnpm/npm audit` (lockfile). Só volta
  com caso real de bundle sem lockfile — mesmo status do G4 (a confirmar).
- **Copiar prompt/skill verbatim**: o texto do SKILL.md é material externo; a
  adaptação é estrutural (seções e guardrails), nunca literal.

## Validação de design de fora

- `SKILL.md`: "no stale snapshot — always reads the latest index" + "Do NOT
  fabricate entries" = **retrieval-first + anti-alucinação** do AGENTS.md, na
  letra.
- O bot de review do repo: a metade LLM morreu (GitHub Models aposentado) e o
  pipeline **continua rodando determinístico**, reportando Depth/Fit como
  "skipped" — é o fail-open com causa do KIRA ("dado ausente aparece como `—`
  com a causa, nunca inventado").
- Detecção de idioma do contribuidor é **regex determinística (Han/Kana)**
  passada ao LLM como diretiva — "o LLM nunca é dono da detecção" = ADR-0016.
- Split privilegiado do CI: quem **avalia** PR de fork roda read-only e
  publica artifact; quem **posta** valida a partir da base confiável
  (allowlist de labels, cap de corpo, SHA do head). É o `awaiting_approval` +
  `kira approve` (ADR-0017): quem lê dado não confiável nunca é quem tem
  poder de escrita.
- "The 200 That Proved Nothing": um gate determinístico reprovou 79/79
  confirmações de um LLM num benchmark de BOLA — a mesma escada do
  ADR-0016/0018 (gate determinístico manda; modelo só explica; sem LLM como
  juiz).

## Ordem de execução

Ver [`kira-w7-web-security.md`](kira-w7-web-security.md) — onda W7 (plano por
PR, gates e aceite).
