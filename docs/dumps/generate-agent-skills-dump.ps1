# Gerador determinístico do dump gitingest dos SKILL.md do agent-skills.
# Por quê: o relatório docs/agent-skills-harness-vale-trazer.md compara prompts do
# agent-skills com o KIRA; o dump dá a evidência versionada (commit SHA) para o
# benchmark. Sair daqui = apontar -RepoDir para o clone e rodar de novo.
#
# Uso: pwsh ./generate-agent-skills-dump.ps1 -RepoDir <clone> -OutDir <docs/dumps/agent-skills>
# Escopo: só SKILL.md (94 arquivos). references/, templates/ e o código do CLI/MCP
# ficam fora (o foco é a riqueza dos prompts; regenere com o clone completo se precisar).
param(
    [Parameter(Mandatory = $true)][string]$RepoDir,
    [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = 'Stop'

$sk = Join-Path $RepoDir 'packages\skills-catalog\skills'
if (-not (Test-Path $sk)) { throw "caminho não encontrado: $sk — aponte -RepoDir para o clone do tech-leads-club/agent-skills" }

$sha = (git -C $RepoDir rev-parse --short HEAD).Trim()
$files = Get-ChildItem $sk -Recurse -File -Filter 'SKILL.md' | Sort-Object FullName
if ($files.Count -eq 0) { throw "nenhum SKILL.md em $sk" }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

function Parse-Frontmatter([string]$content) {
    # Frontmatter YAML simples do SKILL.md: pares "chave: valor" entre os dois '---'
    $meta = @{}
    $in = $false
    foreach ($l in ($content -split "`r?`n")) {
        if ($l -eq '---') { if ($in) { break } else { $in = $true; continue } }
        if ($in -and $l -match '^([A-Za-z_]+):\s*(.*)$') { $meta[$Matches[1]] = $Matches[2].Trim() }
    }
    return $meta
}

$sep = '=' * 64
$records = foreach ($f in $files) {
    $cat = $f.Directory.Parent.Name
    $content = [IO.File]::ReadAllText($f.FullName)
    [pscustomobject]@{
        Cat     = $cat
        Skill   = $f.Directory.Name
        Path    = "packages/skills-catalog/skills/$cat/$($f.Directory.Name)/SKILL.md"
        Meta    = Parse-Frontmatter $content
        Bytes   = $f.Length
        Content = $content
    }
}

# Cada categoria vira um arquivo; ordem determinística por caminho (mesma regra da
# caminhada do KIRA: caminho ordenado, byte-idêntico entre execuções).
$LF = [string]::Join('', @([char]10))
$utf8 = New-Object System.Text.UTF8Encoding($false)
$provenance = @(
    '---',
    'type: Dump',
    "title: gitingest — tech-leads-club/agent-skills",
    "- **Fonte**: https://github.com/tech-leads-club/agent-skills @ commit $sha",
    '- **Licença**: código MIT (Copyright (c) 2026 Tech Leads Club); conteúdo dos SKILL.md',
    '  CC-BY-4.0 — atribuição: Tech Leads Club + autor indicado em `metadata.author` de cada skill.',
    '- **Gerado por**: `docs/dumps/generate-agent-skills-dump.ps1` — caminhada ordenada por caminho;',
    '  mesma entrada = mesmo dump. Fora de escopo: `references/`, `templates/`, código CLI/MCP.',
    '---',
    ''
)

foreach ($group in ($records | Group-Object Cat | Sort-Object Name)) {
    $out = [System.Collections.Generic.List[string]]@()
    $out += $provenance
    $out += ('# gitingest — agent-skills — categoria ' + $group.Name + ' (' + $group.Count + ' skills)')
    $out += ''
    foreach ($r in ($group.Group | Sort-Object Path)) {
        $out += $sep
        $out += ('File: ' + $r.Path)
        $out += $sep
        $out += $r.Content.TrimEnd()
        $out += ''
    }
    $text = [string]::Join($LF, $out)
    [IO.File]::WriteAllText((Join-Path $OutDir (($group.Name -replace '[()]', '') + '.md')), $text, $utf8)
}

# Índice com metadados de todos os 94 skills (versão, autor, tamanho, descrição truncada).
$idx = [System.Collections.Generic.List[string]]@()
$idx += $provenance
$idx += ('# gitingest — agent-skills — índice (' + $records.Count + ' SKILL.md, ' + (($records | Measure-Object Bytes -Sum).Sum) + ' bytes)')
$idx += ''
$idx += '| categoria | skill | versão | autor | bytes | descrição (trecho) |'
$idx += '|---|---|---|---|---|---|'
foreach ($r in ($records | Sort-Object Path)) {
    $d = ''
    if ($r.Meta.ContainsKey('description')) {
        $d = ($r.Meta['description'] -replace '\|', '\|')
        if ($d.Length -gt 160) { $d = $d.Substring(0, 160) + '…' }
    }
    $v = if ($r.Meta.ContainsKey('version')) { $r.Meta['version'] } else { '—' }
    $a = if ($r.Meta.ContainsKey('author')) { $r.Meta['author'] -replace '\|', '\|' } else { '—' }
    $idx += ('| ' + $r.Cat + ' | [' + $r.Skill + '](' + (($r.Cat -replace '[()]', '') + '.md') + ') | ' + $v + ' | ' + $a + ' | ' + $r.Bytes + ' | ' + $d + ' |')
}
$idx += ''
$idx += 'Cada categoria está em um arquivo próprio neste diretório. Skills destacadas na'
$idx += 'análise do KIRA: `create-adr`, `the-judge`, `the-jury`, `the-fool`, `tlc-spec-driven`,'
$idx += '`not-your-babysitter`, `harness-eval`, `spec-driven-eval`.'
[IO.File]::WriteAllText((Join-Path $OutDir 'INDEX.md'), ([string]::Join($LF, $idx) + $LF), $utf8)

# Docs de nível de repo (política de segurança, AGENTS.md deles) — evidência para a
# comparação de governança de prompt.
$repoDocs = [System.Collections.Generic.List[string]]@()
$repoDocs += $provenance
$repoDocs += '# gitingest — agent-skills — documentos do repo'
$repoDocs += ''
foreach ($name in @('README.md', 'SECURITY.md', 'AGENTS.md', 'CLAUDE.md', 'CONTRIBUTING.md')) {
    $p = Join-Path $RepoDir $name
    if (Test-Path $p) {
        $repoDocs += $sep
        $repoDocs += ('File: ' + $name)
        $repoDocs += $sep
        $repoDocs += ([IO.File]::ReadAllText($p).TrimEnd())
        $repoDocs += ''
    }
}
[IO.File]::WriteAllText((Join-Path $OutDir 'repo-docs.md'), ([string]::Join($LF, $repoDocs)), $utf8)

"dump gerado em $OutDir — $($records.Count) SKILL.md, $($files.Count) arquivos lidos, commit $sha"
