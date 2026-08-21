param(
  [Parameter(Mandatory = $true)][string]$BatchPath,
  [Parameter(Mandatory = $true)][string]$LogPath
)

$ErrorActionPreference = 'Stop'
$command = "/d /c call `"$BatchPath`" --elevated > `"$LogPath`" 2>&1"
$process = Start-Process -FilePath $env:ComSpec -Verb RunAs -Wait -PassThru -ArgumentList $command
exit $process.ExitCode
