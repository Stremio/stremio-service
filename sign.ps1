
if ($null -eq (Get-Command "signtool" -ErrorAction SilentlyContinue)) {
    Write-Host "No VC vars found"
}

# Get latest build from s3
Write-Host "Getting latest build from S3..."
$tag = $(git describe --tags --abbrev=0)
aws s3 cp s3://stremio-artifacts/stremio-service-unsigned/$tag/stremio-service-windows.zip .
if (Test-Path "stremio-service-windows") {
    Remove-Item -Recurse -Force .\stremio-service-windows
}
Expand-Archive -Path .\stremio-service-windows.zip -DestinationPath .\stremio-service-windows

Write-Host "Building the installer"
& 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' '/Sstremiosign=$qsigntool.exe$q sign /fd SHA256 /t http://timestamp.digicert.com /n $qSmart Code OOD$q $f' 'setup\StremioService.iss'
signtool sign /fd SHA256 /t http://timestamp.digicert.com /n "Smart Code OOD" *.exe
Write-Host "Done"
