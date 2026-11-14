---
name: tailwind
description: Use when styling with Tailwind CSS utility classes. Do NOT use for plain CSS/SCSS modules or design systems that forbid utilities.
license: MIT
author: KIRA
version: 1.0.0
---
# Tailwind CSS

- **Mobile-first**: escreva o estilo base para o menor viewport e adicione `sm:`/`md:`/`lg:` por cima — nunca o contrário.
- **Tokens**: use a escala do tema (`p-4`, `text-sm`, `rounded-lg`); valor arbitrário (`p-[13px]`) é exceção justificada, não hábito.
- **Ordem**: layout → espaçamento → tipografia → cor/borda → estado (`hover:`, `focus-visible:`, `disabled:`). Mesma ordem em todo o projeto.
- **Componentes**: quando a mesma combinação de classes aparece 3+ vezes, extraia um componente ou uma classe `@apply` nomeada — repetição sem contrato vira divergência.
- **Dark mode**: prefira o tema via `dark:` com tokens semânticos; cor crua espalhada pelo JSX é dívida.
- **Acessibilidade**: `focus-visible` sempre presente; contraste conferido no token, não no olho.
