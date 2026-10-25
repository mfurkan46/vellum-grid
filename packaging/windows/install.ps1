param([string]$Exe = "")
$ErrorActionPreference = "Stop"

if (-not $Exe) {
    $candidates = @(
        (Join-Path $PSScriptRoot "vellum-grid.exe"),
        (Join-Path $PSScriptRoot "..\..\target\release\vellum-grid.exe")
    )
    $Exe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $Exe) { throw "vellum-grid.exe bulunamadı. Yolunu -Exe ile verin." }
}
$Exe = (Resolve-Path $Exe).Path

$dir = Join-Path $env:LOCALAPPDATA "Programs\Vellum Grid"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$target = Join-Path $dir "vellum-grid.exe"
if (Get-Process vellum-grid -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $target }) {
    throw "Vellum Grid açık. Kapatıp yeniden deneyin."
}
Copy-Item -Path $Exe -Destination $target -Force
Copy-Item -Path (Join-Path $PSScriptRoot "uninstall.ps1") -Destination (Join-Path $dir "uninstall.ps1") -Force

$template = Join-Path $dir "Yeni Vellum Grid Çalışma Sayfası.xlsx"
Start-Process -FilePath $target -ArgumentList @("--blank", "`"$template`"") -Wait

$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath("Programs")) "Vellum Grid.lnk"))
$link.TargetPath = $target
$link.IconLocation = "$target,0"
$link.Description = "Vellum Grid"
$link.Save()

function Set-Value([string]$Path, [string]$Name, $Value, [string]$Type = "String") {
    if (-not (Test-Path $Path)) { New-Item -Path $Path -Force | Out-Null }
    if ($Name -eq "") {
        Set-Item -Path $Path -Value $Value
    } else {
        New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType $Type -Force | Out-Null
    }
}

$classes = "HKCU:\Software\Classes"
$progId = "VellumGrid.Workbook"
$icon = "`"$target`",0"
$open = "`"$target`" `"%1`""

Set-Value "$classes\$progId" "" "Vellum Grid Çalışma Sayfası"
Set-Value "$classes\$progId\DefaultIcon" "" $icon
Set-Value "$classes\$progId\shell\open\command" "" $open
Set-Value "$classes\$progId\ShellNew" "FileName" $template

foreach ($ext in @(".xlsx", ".xlsm", ".xls", ".ods", ".csv")) {
    Set-Value "$classes\$ext\OpenWithProgids" $progId ""
    $verb = "$classes\SystemFileAssociations\$ext\shell\VellumGrid"
    Set-Value $verb "" "Vellum Grid ile Aç"
    Set-Value $verb "Icon" $icon
    Set-Value "$verb\command" "" $open
}

$newClass = "VellumGrid.NewWorkbook"
Set-Value "$classes\.vellumgrid" "" $newClass
Set-Value "$classes\$newClass" "" "Vellum Grid Çalışma Sayfası"
Set-Value "$classes\$newClass\DefaultIcon" "" $icon
Set-Value "$classes\.vellumgrid\$newClass\ShellNew" "Command" "`"$target`" --new-file `"%1`""
Set-Value "$classes\.vellumgrid\$newClass\ShellNew" "IconPath" $icon
Remove-Item -Path "$classes\Directory\Background\shell\VellumGridNew" -Recurse -ErrorAction SilentlyContinue
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Discardable\PostSetup\ShellNew" -Recurse -ErrorAction SilentlyContinue

Set-Value "$classes\Applications\vellum-grid.exe" "FriendlyAppName" "Vellum Grid"
Set-Value "$classes\Applications\vellum-grid.exe\shell\open\command" "" $open
Set-Value "$classes\Applications\vellum-grid.exe\DefaultIcon" "" $icon

$capabilities = "HKCU:\Software\Vellum\Vellum Grid\Capabilities"
Set-Value $capabilities "ApplicationName" "Vellum Grid"
Set-Value $capabilities "ApplicationDescription" "Rust ile yazılmış, hızlı ve açık kaynak hesap tablosu"
Set-Value $capabilities "ApplicationIcon" $icon
foreach ($ext in @(".xlsx", ".xlsm", ".xls", ".ods", ".csv")) {
    Set-Value "$classes\Applications\vellum-grid.exe\SupportedTypes" $ext ""
    Set-Value "$capabilities\FileAssociations" $ext $progId
}
Set-Value "HKCU:\Software\RegisteredApplications" "Vellum Grid" "Software\Vellum\Vellum Grid\Capabilities"

$uninstall = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\VellumGrid"
Set-Value $uninstall "DisplayName" "Vellum Grid"
Set-Value $uninstall "DisplayIcon" $icon
Set-Value $uninstall "DisplayVersion" "0.1.0"
Set-Value $uninstall "Publisher" "Vellum"
Set-Value $uninstall "InstallLocation" $dir
Set-Value $uninstall "UninstallString" "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$dir\uninstall.ps1`""
Set-Value $uninstall "NoModify" 1 "DWord"
Set-Value $uninstall "NoRepair" 1 "DWord"

Add-Type -Namespace Vellum -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, uint f, IntPtr a, IntPtr b);'
[Vellum.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)

Write-Output "Vellum Grid kuruldu: $target"
