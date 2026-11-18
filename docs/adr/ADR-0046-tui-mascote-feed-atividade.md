# ADR-0046 — Mascote e feed de atividade: eventos reais, arte de 7 bits, seed FNV-1a

**Status:** Aceito (TUI-01)
**Data:** 2026-11-18

## Contexto

O MVP prometeu feedback visual constante ("o dev vê, em tempo real, o que o
KIRA faz") com mascote pato de borracha determinístico (seed = hash do
projeto) — e persona §7 proíbe barra de progresso falsa. O dashboard já tinha
spinner puro sobre o tick, mas dirigido SÓ por filho rodando; fases
("pesquisando stack…", "pensando…") não existiam. O #151 deu ao daemon saída
ao vivo (`emit`); o feed da TUI passa a consumir os MESMOS eventos (o canal
`RunEvent` do runner, que é o que o ExecState já consome).

## Decisão

**D1 — Animação = reflexo de evento observado, nunca de relógio.** O feed
(`src/tui/activity.rs`) é puro sobre `RunEvent`: spawn (fase nasce do comando
confirmado), linhas do filho (resumo do `scan` no MESMO formato que
`findings_count_in_summary` lê da trilha; linha de tick `tick N: ingest: …` —
o que o `emit` do daemon imprime) e exit (concluído/falhou). Linha
desconhecida não vira fase. Todo texto do feed passa por `redact` antes de
virar tela. Estados do pato: idle/pensando/pesquisando/trabalhando/alerta —
`alerta` = exit ≠ 0 OU `critical > 0` no resumo real; nunca inventado.

**D2 — A arte é de 7 bits, UM conjunto para os dois modos de glifo.** O
critério do modo ASCII (`KIRA_GLYPHS=ascii`, PR-UX1) é que desenho que vira
quadradinho é informação perdida — e isso vale para o pato no modo Unicode
também: em console de fábrica (Windows console, SSH antigo) blocos `█`
desaparecem. O clássico `<(o )___` desenha IGUAL em qualquer terminal; o que
muda com a paleta é a COR (campo `duck` — amarelo na colorida, `Reset` na
mono), nunca o glifo. A animação é função pura do tick (mesmo tick = mesmo
frame), com período por estado (repouso lento, trabalho rápido).

**D3 — Seed = FNV-1a 64 da raiz do projeto, pelo `store::fnv1a64`.** O MESMO
algoritmo do fingerprint do scan (padrão do companheiro do openclaude,
apêndice 10.3). No MVP o seed decide a variante do olho e a fase inicial da
animação; o sistema completo `pick(EYES)`/`rollRarity` fica como evolução, e
o ponto de encaixe é `Duck::eye`.

**D4 — O mascote mora no overlay `o` (faixa de 5 linhas: arte à esquerda,
`fase:` + últimas entradas à direita) e a fase entra no chip `[exec]` do
cartão Sessões.** O log NUNCA fica sob o mascote (faixa separada). Patão na
tela parada não existe: sem filho, sem pato — mostrar mascote idle permanente
na Resumo seria decorar estado que ninguém observou (mesmo critério do chip
`[aguardando]` recusado no PR-UX2).

## Consequências

- Positivo: feedback honesto por fase já na TUI atual; parser de fases é
  função pura e testada contra linhas reais — quando o RESEARCH-01 trouxer
  "chamando skill/ferramenta", o feed consome sem mudar de forma.
- Negativo: durante o `scan` NÃO chega linha até o fim (a ferramenta é
  silenciosa) — a fase "pesquisando stack…" cobre o intervalo inteiro, o que
  é verdade e não barra de progresso.
- Neutro: o feed do DAEMON (processo separado do dashboard) entra num panel
  futuro que consuma a trilha ao vivo; hoje o canal é o runner da casca.
- Visual: a direção cyberpunk do outro agente do dono (`.kombai/`, NUNCA
  limpar) conversa com a TUI futura; este PR não mexe na casca.
