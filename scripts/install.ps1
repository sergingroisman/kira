<#
KIRA — instalador para Windows com verificação de integridade (SHA-256).

O equivalente do scripts/install.sh, porque o desenvolvedor do KIRA usa Windows:
aqui não existe `sha256sum`, existe `Get-FileHash`. O fluxo é o mesmo — baixar,
conferir o hash publicado e SÓ ENTÃO colocar o binário no PATH. Sem admin, sem
winget, sem instalação silenciosa: nada fora da pasta do usuário.

NUNCA execute este script via pipe (`irm … | iex` ou `curl … | powershell`):
o pipe cego executa o que chegar, sem chance de conferir. Fluxo seguro:

  Set-ExecutionPolicy -Scope Process Bypass   # só neste processo (PR14)
  .\install.ps1 [-WithModel <repo>] [-Yes]

Uso:
  install.ps1                       # só o binário do KIRA
  install.ps1 -WithModel <repo>     # + download do modelo (opt-in, nunca
                                    #  silencioso — mesmo contrato do
                                    #  `kira model pull`: -Yes OU prompt,
                                    #  destino + disco livre medidos ANTES
                                    #  de qualquer download/spawn)
  install.ps1 -Yes                  # atalho de consentimento (script/CI)

O Ollama NUNCA é instalado daqui: ausente ⇒ instrução manual e saída limpa —
instalar dependência de IA silenciosamente é exatamente o que este script
existe para não fazer. Piso de disco para -WithModel: env KIRA_MIN_FREE_MB
(default 4096 MB — qwen3.5:4b Q4_K_M ≈ 3,3 GB + margem). Agendamento NÃO
entra aqui (regra do roadmap PR14).

Fronteira honesta: o SHA256SUMS vem do mesmo host do binário, então isto garante
integridade de transporte, não autoria. Para autoria, fixe o hash fora do canal
via -PinnedSha256.
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = $(if ($env:KIRA_BASE_URL) { $env:KIRA_BASE_URL } else { "https://github.com/sergingroisman/kira/releases/latest/download" }),
    [string]$InstallDir = $(if ($env:KIRA_INSTALL_DIR) { $env:KIRA_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "kira\bin" }),
    [string]$PinnedSha256 = $env:KIRA_PINNED_SHA256,
    # Opt-in do modelo: sem -WithModel NENHUM download acontece — o instalador
    # só coloca o binário do KIRA no disco (formato do Ollama:
    # `hf.co/<org>/<repo>:Q4_K_M` ou tag local como `qwen3.5:4b`).
    [string]$WithModel = "",
    # Consentimento explícito do download do modelo (o pull não é silencioso).
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

function Write-Note($text) { Write-Host "  $text" }
function Stop-Install($text) { Write-Error "erro: $text"; exit 1 }

# Piso de disco para -WithModel (mesma env do install.sh; inteiro em MB).
$MinFreeMb = 4096
if ($env:KIRA_MIN_FREE_MB) {
    if ($env:KIRA_MIN_FREE_MB -notmatch '^\d+$') {
        Stop-Install "KIRA_MIN_FREE_MB='$($env:KIRA_MIN_FREE_MB)' não é um inteiro (MB) — fixe um valor numérico"
    }
    $MinFreeMb = [int]$env:KIRA_MIN_FREE_MB
}

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

    # --- Ollama: detectar e orientar, NUNCA instalar --------------------------
    # O Ollama é o backend de IA local (default eterno) e quem baixa/executa
    # modelos (ADR-0040). Este script nunca o instala: ausente ⇒ instrução
    # manual e saída limpa — instalar dependência de IA silenciosamente é
    # exatamente o que este script existe para não fazer. A override
    # KIRA_MODEL_OLLAMA_BIN espelha o contrato do `kira model pull` (HF-2).
    $ollamaBin = $null
    if ($env:KIRA_MODEL_OLLAMA_BIN) {
        # Override presente é contrato do usuário: arquivo válido é usado como
        # veio; inválido conta como ausente.
        if (Test-Path $env:KIRA_MODEL_OLLAMA_BIN -PathType Leaf) {
            $ollamaBin = $env:KIRA_MODEL_OLLAMA_BIN
        }
    } else {
        $cmd = Get-Command ollama -ErrorAction SilentlyContinue
        if ($cmd) { $ollamaBin = $cmd.Source }
    }

    if (-not $ollamaBin) {
        Write-Host ""
        Write-Host "Ollama não encontrado — o KIRA funciona, mas o backend de IA local não."
        Write-Note "instale manualmente: https://ollama.com (este script não instala nada silenciosamente)"
        Write-Note "depois rode: kira setup   (o kira doctor detecta o ollama)"
        if ($WithModel) {
            Write-Note "-WithModel $WithModel NÃO baixado: sem ollama não há pull — instale o Ollama e rode 'kira model pull $WithModel'"
        }
    } elseif ($WithModel) {
        # --- -WithModel: mesmo contrato do `kira model pull` (HF-2) -----------
        # Disco medido ANTES de qualquer consentimento/spawn; consentimento é
        # estado (-Yes OU prompt); recusa em qualquer um = nada baixado.
        $dest = if ($env:OLLAMA_MODELS) { $env:OLLAMA_MODELS } else { Join-Path $env:USERPROFILE ".ollama\models" }

        # Get-PSDrive .Free = inteiro puro, invariante de cultura (mesma
        # leitura do free_disk_bytes no model.rs). Falha ⇒ nunca zero fabricado.
        $drive = (Resolve-Path $dest -ErrorAction SilentlyContinue).PSDrive
        $freeBytes = $null
        if ($drive) {
            try { $freeBytes = [long](Get-PSDrive $drive.Name).Free } catch { $freeBytes = $null }
        }

        # Disco é limite físico, não consentimento: sem medição ou abaixo do
        # piso, recusa ANTES de qualquer spawn — mesmo com -Yes.
        if (-not $freeBytes) {
            Stop-Install "não consegui medir o disco livre de $dest — NADA foi baixado; confira o destino (OLLAMA_MODELS?) e rode de novo"
        }
        $requiredBytes = [long]$MinFreeMb * 1MB
        if ($freeBytes -lt $requiredBytes) {
            Stop-Install "espaço livre insuficiente em $dest: $([math]::Round($freeBytes / 1MB)) MB livre, piso ${MinFreeMb} MB (KIRA_MIN_FREE_MB) — NADA foi baixado; libere espaço ou rode 'ollama pull $WithModel' por sua conta"
        }

        if (-not $Yes) {
            Write-Host "baixar modelo do Hugging Face? $WithModel"
            Write-Note "destino: $dest"
            Write-Note "espaço livre: $([math]::Round($freeBytes / 1MB)) MB"
            Write-Note "o Ollama mostra o tamanho durante o download"
            try { $answer = Read-Host "confirmar [y/N]" } catch { $answer = "" }
            # EOF/Enter vazio/qualquer outra coisa = recusa (nunca baixa).
            if ($answer -notmatch '^(?i)(y|yes|s|sim)$') {
                Stop-Install "download cancelado — nada foi baixado"
            }
        }

        & $ollamaBin pull $WithModel
        if ($LASTEXITCODE -ne 0) {
            Stop-Install "o 'ollama pull $WithModel' falhou (exit $LASTEXITCODE) — rode o pull direto para ver o erro completo e rode de novo"
        }
        Write-Host "modelo baixado: $WithModel (destino $dest)"
    }

    # --- Smoke: `kira doctor` no fim — aviso ≠ erro ---------------------------
    # Em instalação nova o doctor SEMPRE sinaliza pendências (PAT, config,
    # ollama ausente): estados esperados pós-instalação, não defeito do
    # install. O binário já provou que executa (--version acima); o smoke é
    # diagnóstico impresso, nunca decisão de falha do instalador. Exit code
    # de exe nativo NÃO lança exceção — a decisão é pelo $LASTEXITCODE; a
    # preferência Stop desce só aqui porque `2>&1` de exe nativo viria como
    # NativeCommandError e mataria o smoke por um aviso.
    Write-Host ""
    Write-Host "smoke: kira doctor"
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { & $destino doctor 2>&1 | Write-Host } catch { Write-Host $_.Exception.Message }
    $doctorExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    if ($doctorExit -eq 0) {
        Write-Note "doctor ok"
    } else {
        Write-Note "doctor sinalizou pendências — esperado em instalação nova (PAT/config); o binário funciona e o install está completo"
    }
} finally {
    Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
