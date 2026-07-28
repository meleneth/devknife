param(
    [string] $InstallDir = "",
    [string] $CommandName = "devknife"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$desktopDir = Join-Path $repoRoot "apps\desktop"
$sourceExe = Join-Path $repoRoot "target\release\devknife-desktop.exe"

if (-not $InstallDir) {
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
    $cargoBin = Join-Path $cargoHome "bin"
    if (Test-Path $cargoBin) {
        $InstallDir = $cargoBin
    } else {
        $InstallDir = Join-Path $env:USERPROFILE ".local\bin"
    }
}

$targetExe = Join-Path $InstallDir "$CommandName.exe"

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm is required to build the desktop app."
}

Write-Host "Installing desktop dependencies..."
npm --prefix $desktopDir install

Write-Host "Building desktop executable..."
npm --prefix $desktopDir run package

if (-not (Test-Path $sourceExe)) {
    throw "Expected built executable at $sourceExe, but it was not found."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force -Path $sourceExe -Destination $targetExe

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()
if ($userPath) {
    $pathEntries = $userPath -split ';' | Where-Object { $_ }
}

$alreadyOnPath = $pathEntries | Where-Object {
    [string]::Equals(
        $_.TrimEnd('\'),
        $InstallDir.TrimEnd('\'),
        [StringComparison]::OrdinalIgnoreCase
    )
}

if (-not $alreadyOnPath) {
    $newUserPath = (@($pathEntries) + $InstallDir) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    Write-Host "Added $InstallDir to your user PATH. Open a new terminal before running $CommandName."
} else {
    Write-Host "$InstallDir is already on your user PATH."
}

Write-Host "Installed $CommandName at $targetExe"
