# @path/file — Guia de Uso

## Funcionalidade Implementada

O Kira agora suporta anexar arquivos e imagens ao input usando a sintaxe `@path/file`, similar ao Claude Code.

### Recursos

1. **@path/file** — Anexar arquivos e imagens ao input
2. **Autocomplete** — Dropdown de sugestões ao digitar `@`
3. **Cursor Navigation** — Navegação completa no input field
4. **Fake Tool Use/Result** — LLM recebe contexto como se tivesse lido o arquivo

## Como Usar

### Autocomplete

1. Digite `@` seguido de um path parcial:
   ```
   analisa @./src/
   ```
2. Um dropdown aparece com sugestões de arquivos/diretórios
3. Use `↑↓` para navegar nas sugestões
4. Pressione `Tab` para aceitar a sugestão selecionada
5. Para diretórios, o dropdown continua mostrando o conteúdo
6. Para arquivos, um espaço é adicionado após a seleção

### Arquivos de Texto

```
analisa @./src/router/mod.rs e me diz o que faz
```

```
refatora @./Cargo.toml para adicionar dependências
```

### Imagens

```
o que tem @./docs/architecture.png
```

### Line Ranges

```
analisa @./src/router/mod.rs#L50-80
```

### Paths com Espaços

```
analisa @"./my folder/config.toml"
```

### Home Directory

```
analisa @~/.config/kira/config.toml
```

### Multi-file

```
compara @./src/router/mod.rs com @./src/server/chat.rs
```

## Navegação no Input

| Tecla | Ação |
|-------|------|
| `←/→` | Mover cursor |
| `Ctrl+←/→` | Pular palavras |
| `Home` / `Ctrl+A` | Ir para início |
| `End` / `Ctrl+E` | Ir para fim |
| `Backspace` | Deletar antes do cursor |
| `Delete` | Deletar depois do cursor |
| `Ctrl+Backspace` | Deletar palavra antes |
| `Ctrl+Delete` | Deletar palavra depois |
| `Ctrl+K` | Deletar até o fim |
| `Ctrl+U` | Deletar até o início |
| `↑↓` | Histórico (ou navegar sugestões) |
| `Tab` | Aceitar sugestão de autocomplete |
| `Escape` | Limpar input |
| `Ctrl+C` (1x) | Limpar input |
| `Ctrl+C` (2x) | Sair do REPL |

## Comportamento

1. **Detecção**: O parser detecta `@` seguido de um path no input
2. **Autocomplete**: Dropdown aparece com sugestões de arquivos
3. **Resolução**: Paths relativos são resolvidos a partir do diretório atual
4. **Leitura**: O conteúdo do arquivo é lido e injetado como fake tool_use/tool_result
5. **Display**: Anexos aparecem como linhas dim: `Read file.rs (342 lines)`
6. **Envio**: O LLM recebe o contexto como se ele mesmo tivesse lido o arquivo

## Limites

- **Texto**: 50KB máximo, 500 linhas máximo
- **Imagens**: 5MB máximo
- **Tipos suportados**: .rs, .md, .toml, .yaml, .json, .ts, .tsx, .js, .py, .sh, .png, .jpg, .gif, .svg, etc.
- **Autocomplete**: Máximo 20 sugestões, cache de 5 segundos

## Segurança

- Paths fora do workspace ou home são rejeitados
- Truncation automática para arquivos grandes
- Validação de tipo de arquivo
- Arquivos ocultos ignorados a menos que explicitamente solicitados

## Exemplos de Uso

### YOLO Mode

```
/mode yolo
analisa @./src/router/mod.rs e sugere melhorias
```

### Plan Mode

```
/mode plan
gera documentação para @./src/router/mod.rs
```

### Com Autocomplete

```
# Digite: analisa @./src/
# Dropdown aparece com:
#   📁 router/
#   📁 server/
#   📄 main.rs
# Pressione ↓ para selecionar router/
# Pressione Tab para aceitar
# Novo dropdown aparece com conteúdo de router/
# Selecione mod.rs e pressione Tab
# Input final: "analisa @./src/router/mod.rs "
# Pressione Enter para enviar
```

### With Image

```
analisa @./src/router/mod.rs e gera um diagrama baseado em @./docs/arch.png
```
