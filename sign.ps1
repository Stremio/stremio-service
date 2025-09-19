param (
    [String]$pw = $( Read-Host "Password" )
)

if ($null -eq (Get-Command "signtool" -ErrorAction SilentlyContinue)) {
    Write-Host "No VC vars found"
}

$thread = Start-ThreadJob -InputObject ($pw) -ScriptBlock {
    $wshell = New-Object -ComObject wscript.shell;
    $pw = "$($input)~"
    while ($true) {
        while ( -not $wshell.AppActivate("Token Logon")) {
            Start-Sleep 1
        }
        Start-Sleep 1
        $wshell.SendKeys($pw, $true)
        Start-Sleep 1
    }
}

# Get latest build from s3
Write-Host "Getting latest build from S3..."
$tag = $(git describe --tags --abbrev=0)
aws s3 cp s3://stremio-artifacts/stremio-service-unsigned/$tag/stremio-service-windows.zip .
if (Test-Path "stremio-service-windows") {
    Remove-Item -Recurse -Force .\stremio-service-windows
}
Expand-Archive -Path .\stremio-service-windows.zip -DestinationPath .\stremio-service-windows

$env:package_version = (Select-String -Path .\CMakeLists.txt -Pattern '^project\(stremio VERSION "([^"]+)"\)').Matches.Groups[1].Value
Write-Host "Building the installer"
& 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' '/Sstremiosign=$qsigntool.exe$q sign /fd SHA256 /t http://timestamp.digicert.com /n $qSmart Code OOD$q $f' 'setup\StremioService.iss'

Stop-Job $thread
Write-Host "Done"
