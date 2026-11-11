# ADR-0022 — Organização por domínio agora, camadas quando houver código compartilhado

- **Status**: aceito (decisão do dono, 11/Nov/2026)
- **Decisores**: Sérgio (dono)

## Contexto

O dono descreveu preferência por organização **por camada** (`domain/`, `infra/`), mas o
código atual é **por domínio** (`src/ado/`, `src/scan/`, `src/wiki/`, `src/store/`,
`src/context/`, `src/llm/`) e essa divisão hoje coincide exatamente com as fronteiras
reais do produto. Migrar agora seria churn: renomear ~30 arquivos, reescrever imports e
arriscar regressão em código que está verde, sem nenhum ganho funcional imediato.

## Decisão

1. **Manter por domínio** enquanto cada módulo tiver um dono claro e não houver código
   compartilhado entre eles.
2. **Introduzir camadas sob demanda**, e só com evidência: quando duas ou mais features
   precisarem da mesma abstração (ex.: um `domain::Tool` usado por scan e por fix).
3. A introdução de camadas é uma decisão de ADR própria, com o diff proposto no PR.

## Consequências

- **Positivas**: zero churn agora; fronteiras continuam óbvias (`ado` não importa `scan`);
  teste e revisão seguem baratos.
- **Negativas**: quando o código compartilhado aparecer, a migração custa mais tarde —
  mitigado pelo item 3 (a decisão fica registrada antes de virar dívida).
- **Critério de reavaliação**: dois módulos passando a importar a mesma lógica de negócio,
  ou um `mod.rs` de domínio passando de ~800 linhas com responsabilidades misturadas.
