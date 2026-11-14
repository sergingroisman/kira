---
name: shadcn
description: Use when adding shadcn/ui components (dialog, form, table, toast). Do NOT use when the project uses another component library or has no component registry.
license: MIT
author: KIRA
version: 1.0.0
---
# shadcn/ui

- **Código no projeto**: componentes entram no repositório (`components/ui/`) e são SEUS — edite à vontade, mas mantenha o contrato de props consistente entre usos.
- **Adicione o que usa**: um `dialog` de cada vez; importar o catálogo inteiro infla o bundle e cria código morto.
- **Formulário**: `Form` + `FormField` com o schema de validação na fronteira (zod/react-hook-form) — validação duplicada no handler é dívida.
- **Acessibilidade**: não remova `DialogTitle`/`DialogDescription` (o Radix depende deles para leitores de tela); foco e `Esc` vêm do primitivo, não reimplemente.
- **Tema**: cores via tokens CSS (`--primary`, `--muted`), nunca hex solto no componente.
