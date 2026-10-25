$ErrorActionPreference = "SilentlyContinue"

$classes = "HKCU:\Software\Classes"
$progId = "VellumGrid.Workbook"

Remove-Item -Path "$classes\$progId" -Recurse
foreach ($ext in @(".xlsx", ".xlsm", ".xls", ".ods", ".csv")) {
    Remove-ItemProperty -Path "$classes\$ext\OpenWithProgids" -Name $progId
    Remove-Item -Path "$classes\SystemFileAssociations\$ext\shell\VellumGrid" -Recurse
}
Remove-Item -Path "$classes\Directory\Background\shell\VellumGridNew" -Recurse
Remove-Item -Path "$classes\.vellumgrid" -Recurse
Remove-Item -Path "$classes\VellumGrid.NewWorkbook" -Recurse
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Discardable\PostSetup\ShellNew" -Recurse
Remove-Item -Path "$classes\Applications\vellum-grid.exe" -Recurse
Remove-Item -Path "HKCU:\Software\Vellum\Vellum Grid" -Recurse
Remove-ItemProperty -Path "HKCU:\Software\RegisteredApplications" -Name "Vellum Grid"
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\VellumGrid" -Recurse

Remove-Item -Path (Join-Path ([Environment]::GetFolderPath("Programs")) "Vellum Grid.lnk")
Remove-Item -Path (Join-Path $env:LOCALAPPDATA "Programs\Vellum Grid") -Recurse -Force

Add-Type -Namespace Vellum -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, uint f, IntPtr a, IntPtr b);'
[Vellum.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)

Write-Output "Vellum Grid kaldırıldı. Kullanıcı ayarları ve son kullanılanlar listesi %APPDATA%\Vellum Grid altında kalır."
