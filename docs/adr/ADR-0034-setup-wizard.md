# ADR-0034 — `kira setup` ganha wizard de tela cheia (coleta) sobre o mesmo motor de texto

> Renumerado em 12/Nov/2026: era `ADR-0030` na árvore local até a PR #64 reservar `0029`
> (vocabulário de comandos) e `0030` (local-first). Nenhum merge existiu com o número velho.

- **Status**: implementado — aguarda aceite do dono · **Data**: 12/Nov/2026
- **Relaciona**: ADR-0025 (setup ensina) · ADR-0035 (Foundry no wizard)
  · ADR-0014/ADR-0033 (casca do dashboard)
  · layout do onboarding do opencode (MIT, só padrão de interação)
  · jornada completa em [`docs/ux-setup-jornada.md`](../ux-setup-jornada.md)

## Contexto

O `kira setup` cumpre o contrato de ensinar (ADR-0025), mas o dono relatou atrito de instalação nos
três SOs: parede de texto antes de qualquer input, etapas invisíveis, ordem confusa (nome do dev
depois da validação do PAT), nada que mostre o que a máquina já tem, e — pior — o texto prometia
"a entrada não aparece na tela" mesmo no Windows, onde o token era ecoado.

O ingest do opencode (MIT) mostrou um caminho melhor: trilha de etapas à esquerda, painel à direita,
rodapé de ações, detecção de ambiente antes de perguntar e "pular" como opção explícita.

## Decisão

1. **Dois modos, decididos por função pura** (`setup::wizard::mode`): `Wizard` só com TTY nos dois
   lados, terminal ≥80×24, `--plain` ausente e nenhuma flag explícita (`--pat`, `--org`, `--project`,
   `--backend`, `--yes`, `--skip`, `--reset`). Qualquer dúvida ⇒ `Plain`, que é o modo do CI e dos
   testes de hoje.
2. **O wizard COLETA, não executa.** Ele devolve `SetupOptions` com `interactive: false`; quem valida
   o PAT, grava `config.toml`/keyring e verifica a IA continua sendo `setup::run` — uma única fonte
   de verdade para efeitos, mensagens e fail-open.
3. **Etapas**: Ambiente (detecção local: SO, arquitetura, shell, keyring compilado, caminhos de
   config/store, ferramentas no PATH) → Azure (org/projeto, com Tab entre campos) → Acesso
   (colar/ambiente/pular; campo mascarado) → IA (ollama/copilot/foundry) → Resumo (o que será
   gravado, com a frase "nada é gravado antes do Enter").
4. **Segredo sem eco nos três SOs**: o wizard roda em crossterm raw mode, que desliga o eco do
   terminal por construção (no Windows inclusive) — sem `stty` e sem dependência nova. A tela mostra
   `••••••••••••` de tamanho fixo (o comprimento real não vaza) e o valor nunca é renderizado.
   No modo texto, o prompt passa a dizer a verdade: onde não há mecanismo, avisa que o valor pode
   ecoar.
5. **Correções multi-SO que vieram junto**: feature `apple-native-keyring-store` (macOS deixa de cair
   no env em runtime), dicas de variável na sintaxe do shell (`$env:`/`setx` no Windows, `export` no
   Unix), remoção do comando fantasma `kira setup --deps --only font` do resumo, e `hidden_input`
   passa a ser respeitado (o `stty` não roda mais em teste).
6. **Descartado**: modo "wizard" sem TTY (canal de texto é o CI), animação/shimmer do opencode,
   temas JSON, fluxo OAuth por navegador e qualquer dependência de UI nova.

## Consequências

- Positivas: primeiro contato deixa de ser uma parede de texto; o dono vê o que a máquina já tem e
  não responde o que o KIRA pode detectar; segredo realmente oculto no Windows; macOS com keyring.
- Negativas: mais um caminho de UI para manter (o modo texto segue como referência testável) e o
  wizard não é coberto por teste de terminal (só `WizardState` puro + render em `TestBackend`).
- Riscos: `q` dentro do passo de Acesso é tratado como cancelamento apenas fora do campo do segredo,
  para um PAT colado nunca ser interpretado como saída.

## Testes que fixam a decisão

- `mode_only_opens_the_wizard_in_a_big_tty_without_explicit_flags` (unit, `src/setup/wizard.rs`).
- `secret_is_masked_and_never_appears_in_state_strings` e
  `render_shows_the_rail_the_panel_and_never_the_secret` (TestBackend).
- `digits_type_into_the_pat_instead_of_picking_menu_items` (PAT tem dígitos: `1/2/3` não são menu).
- `secret_prompt_only_claims_hiding_when_the_os_can_do_it` (unit, `src/setup.rs`).
