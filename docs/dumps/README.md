# Dumps — gitingest e extrações (fonte de evidência dos relatórios)

Arquivos de análise gerados deterministicamente para sustentar relatórios de
reaproveitamento (`docs/*-vale-trazer.md`, ADRs). Dump não é knowledge base: é
evidência pontada por commit SHA; regenerável, descartável.

## agent-skills/ — COMITADO (MIT + CC-BY-4.0)

Dump gitingest dos **94 SKILL.md** de `tech-leads-club/agent-skills` @
`e74dc8a` + docs de nível de repo (README, SECURITY, AGENTS, CLAUDE,
CONTRIBUTING). Licença: MIT para código; **CC-BY-4.0 para o conteúdo dos
SKILL.md** — uso aqui é análise com atribuição (Tech Leads Club + autor do
frontmatter `metadata.author`). Fora do escopo: `references/`, `templates/` e o
código do CLI/MCP.

- Regenerar: `powershell -ExecutionPolicy Bypass -File
  generate-agent-skills-dump.ps1 -RepoDir <clone> -OutDir <este dir>` (caminhada
  ordenada por caminho; mesma entrada = mesmo dump). Nota de operação: o script
  precisa de UTF-8 **com BOM** para o Windows PowerShell 5.1 não ler os acentos
  como ANSI (o mesmo bug de BOM que o ADR-0023-P1 corrige no scan).

## harness-toolkit-extracao.md — extração (o repo é Elastic License 2.0)

Paráfrase estruturada escrita do zero, com citações curtas atribuídas. O **dump
cru** (624 arquivos, 4,2 MB @ `d293c7f`) fica **LOCAL** (`%TEMP%/opencode/dumps/
gitingest-harness-toolkit.md`) — ELv2 não entra no repo MIT (mesma regra do
"conceito sim, código não" do RTK/BUSL). Quem precisar do cru: `git clone
--depth 1 https://github.com/tech-leads-club/harness-toolkit` e gere com o
gitingest de sempre.
