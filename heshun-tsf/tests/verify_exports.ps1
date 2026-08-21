param(
  [Parameter(Mandatory = $true, Position = 0)][string]$DllPath
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $DllPath)) { throw "DLL does not exist: $DllPath" }

$dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
if ($null -eq $dumpbin) { throw 'dumpbin.exe is not on PATH; run from a VS x64 developer prompt.' }
$exports = & $dumpbin.Source /exports $DllPath 2>&1 | Out-String
foreach ($name in @('DllCanUnloadNow', 'DllGetClassObject', 'DllRegisterServer', 'DllUnregisterServer')) {
  if ($exports -notmatch [regex]::Escape($name)) { throw "Missing COM export: $name" }
}
Write-Host 'TSF COM exports: OK'
