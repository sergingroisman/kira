<#
KIRA — instalador para Windows com verificação de integridade (SHA-256).

O equivalente do scripts/install.sh, porque o desenvolvedor do KIRA usa Windows:
aqui não existe `sha256sum`, existe `Get-FileHash`. O fluxo é o mesmo — baixar,
conferir o hash publicado e SÓ ENTÃO colocar o binário no PATH. Sem admin, sem
winget, sem instalação silenciosa: nada fora da pasta do usuário.

Fronteira honesta: o SHA256SUMS vem do mesmo host do binário, então isto garante
integridade de transporte, não autoria. Para autoria, fixe o hash fora do canal
via -PinnedSha256.
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = $(if ($env:KIRA_BASE_URL) { $env:KIRA_BASE_URL } else { "https://github.com/sergingroisman/kira/releases/latest/download" }),
    [string]$InstallDir = $(if ($env:KIRA_INSTALL_DIR) { $env:KIRA_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "kira\bin" }),
    [string]$PinnedSha256 = $env:KIRA_PINNED_SHA256
)

$ErrorActionPreference = "Stop"

function Write-Note($text) { Write-Host "  $text" }
function Stop-Install($text) { Write-Error "erro: $text"; exit 1 }

# --- Plataforma ---------------------------------------------------------------
$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x86_64" }
    "ARM64" { "aarch64" }
    default { Stop-Install "arquitetura não suportada: $($env:PROCESSOR_ARCHITECTURE)" }
}
$target = "kira-$arch-pc-windows-msvc.zip"

# --- Download -----------------------------------------------------------------
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("kira-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
    Write-Host "KIRA — baixando $target"
    $artifactPath = Join-Path $tmp $target
    $sumsPath = Join-Path $tmp "SHA256SUMS"

    try {
        Invoke-WebRequest -Uri "$BaseUrl/$target" -OutFile $artifactPath -UseBasicParsing
        Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile $sumsPath -UseBasicParsing
    } catch {
        Stop-Install "não consegui baixar da release ($BaseUrl): $($_.Exception.Message)"
    }

    # --- Verificação ----------------------------------------------------------
    $expected = $null
    foreach ($line in Get-Content $sumsPath) {
        $parts = $line -split '\s+', 2
        if ($parts.Count -eq 2) {
            $name = $parts[1].TrimStart('*')
            if ($name -eq $target) { $expected = $parts[0]; break }
        }
    }
    if (-not $expected) { Stop-Install "o SHA256SUMS publicado não lista $target — release incompleta" }

    if ($PinnedSha256 -and $PinnedSha256 -ne $expected) {
        Stop-Install "o hash publicado ($expected) difere do fixado ($PinnedSha256) — pare e verifique a release"
    }

    $actual = (Get-FileHash -Path $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected.ToLowerInvariant()) {
        Stop-Install "VERIFICAÇÃO FALHOU — esperado $expected, obtido $actual. Nada foi instalado."
    }
    Write-Note "sha256 ok: $actual"

    # --- Extração e instalação ------------------------------------------------
    Expand-Archive -Path $artifactPath -DestinationPath $tmp -Force

    $binary = Get-ChildItem -Path $tmp -Filter "kira.exe" -Recurse -File |
        Select-Object -First 1
    if (-not $binary) { Stop-Install "o pacote não contém kira.exe" }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $destino = Join-Path $InstallDir "kira.exe"
    Copy-Item -Path $binary.FullName -Destination $destino -Force

    try { & $destino --version | Out-Null } catch {
        Stop-Install "instalei $destino mas ele não executa — pacote incompatível com esta máquina"
    }

    Write-Host ""
    Write-Host "KIRA instalado em $destino"

    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($userPath -notlike "*$InstallDir*") {
        Write-Note "atenção: $InstallDir não está no PATH do usuário."
        Write-Note "adicione (sem admin, só para você):"
        Write-Note "  [Environment]::SetEnvironmentVariable('PATH', `"$InstallDir;`$env:PATH`", 'User')"
        Write-Note "depois abra um terminal novo e rode: kira setup"
    } else {
        Write-Note "pronto para usar: kira setup"
    }
} finally {
    Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
