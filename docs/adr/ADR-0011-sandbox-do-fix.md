# ADR-0011 — Sandbox do `kira fix --apply`: reimplementar o mínimo em Rust

- **Status**: aceito (decisão do dono, 10/Nov/2026)
- **Contexto**: estudo dos projetos do Akita (ai-jail, ai-memory, ai-usagebar)
- **Decisores**: Sérgio (dono) · Orchestrator (Hermes)

## Contexto

O `kira fix --apply` (M3/W5) vai escrever no repo do usuário. O estudo do
`ai-jail` mostrou a tese correta: defender-se de um agente **confiável mas
falível**, com o diretório do projeto como **raio de explosão** — deny-list de
caminho é a camada 1, não a solução (`rm -rf ../` passa por qualquer lista de
nomes proibidos).

Três caminhos foram avaliados:

- **(A)** invocar `ai-jail` como binário externo opcional;
- **(B)** reimplementar o mínimo em Rust dentro do KIRA;
- **(C)** híbrido (deny-list interno + jail externo quando presente no PATH).

Fatos que pesaram contra (A) e (C):

1. `ai-jail` é **GPL-3.0**; o KIRA é **MIT**. Copiar código contamina a licença;
   invocar o binário não contamina, mas cria dependência de terceiro.
2. `ai-jail` **não tem backend nativo no Windows** — o próprio repositório publica
   `docs/windows-port-assessment.md` recomendando WSL2. O dono é **Windows-first**
   (RTX 2060), então um caminho que depende do ai-jail contradiz o alvo principal.
3. A tese do produto é **binário único, zero instalação**. Dependência externa
   obrigatória quebra a tese.

## Decisão

**(B) Reimplementar o mínimo em Rust, dentro do KIRA, sem bwrap/Landlock no MVP.**

Camadas, em ordem de implementação:

1. **deny-list de escrita** (`src/guard/path.rs`, W1-PR3): `.git/`, `target/`,
   `.kira/` fora do root, `~/.config/kira/`, caminhos absolutos de sistema.
   Erro acionável, nunca toca o disco.
2. **confinamento de diretório**: todo caminho de escrita é canonicalizado e
   verificado contra o root do projeto **depois** da canonicalização (mata
   `..`, symlink e junction). Só `std` — sem dependência nova.
3. **rlimits** (`cfg(unix)`): teto de CPU, memória e número de processos por
   processo filho. Justificativa de dependência: `libc`, já presente na árvore
   transitiva (rusqlite/azure), é a única forma portátil de aplicar `setrlimit`.
4. **Windows**: sem `rlimits` no MVP. Confinamento por canonicalização + deny-list
   valem igualmente; Job Objects (teto de memória/processos) fica registrado como
   item futuro, não como promessa.

## Consequências

- **Positivas**: licença preservada (MIT); funciona no Windows (alvo do dono); sem
  instalação externa; código pequeno e testável com `tempdir`, sem privilégio.
- **Negativas**: não é isolamento de kernel. Um processo que já roda código
  arbitrário pode sair do diretório por mecanismos que `std` não controla. Isso é
  aceitável porque o adversário do modelo é **erro de agente**, não atacante
  motivado — a mesma premissa declarada pelo ai-jail.
- **Limite honesto a documentar no README**: "accident-prevention, não sandbox de
  segurança"; para código hostil, máquina virtual.
- **Invariante**: `fix --apply` continua exigindo `awaiting_approval` aprovado por
  `kira approve <id>`. O jail é a segunda barreira, nunca a única.

## Alternativas rejeitadas

- **(A) invocar ai-jail**: sem Windows nativo; dependência de terceiro GPL-3.0.
- **(C) híbrido**: mantém a dependência externa como caminho "preferido" e cria
  dois comportamentos diferentes por SO — divergência que o CI 3 SOs teria de
  carregar para sempre.
- **bwrap/Landlock no MVP**: Linux-only, exige privilégio e quebra os outros dois
  SOs do CI. Fica como melhoria opcional futura, atrás de feature flag.
