use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct Printers {
    names: Arc<Mutex<Option<Vec<String>>>>,
}

impl Printers {
    pub fn names(&self) -> Option<Vec<String>> {
        let mut slot = self.names.lock().expect("printer list lock");
        if slot.is_none() {
            *slot = Some(Vec::new());
            let names = Arc::clone(&self.names);
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let found = list();
                *names.lock().expect("printer list lock") = Some(found);
            });
            #[cfg(target_arch = "wasm32")]
            drop(names);
            return None;
        }
        slot.clone()
    }
}

#[cfg(target_os = "windows")]
fn hidden(command: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(target_os = "windows")]
fn list() -> Vec<String> {
    let script = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
                  Get-CimInstance Win32_Printer | ForEach-Object { $_.Name }";
    let output = hidden(&mut std::process::Command::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output();
    lines(output)
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
fn list() -> Vec<String> {
    let output = std::process::Command::new("lpstat").arg("-a").output();
    lines(output)
        .into_iter()
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn lines(output: std::io::Result<std::process::Output>) -> Vec<String> {
    output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
pub fn print(file: &Path, printer: &str) -> bool {
    let quote = |s: &str| s.replace('\'', "''");
    let script = format!(
        "Start-Process -FilePath '{}' -Verb PrintTo -ArgumentList '\"{}\"' -WindowStyle Hidden",
        quote(&file.display().to_string()),
        quote(printer)
    );
    hidden(&mut std::process::Command::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
pub fn print(file: &Path, printer: &str) -> bool {
    std::process::Command::new("lp")
        .args(["-d", printer])
        .arg(file)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_arch = "wasm32")]
pub fn print(_file: &Path, _printer: &str) -> bool {
    false
}

pub fn open(path: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_arch = "wasm32")]
    let _ = path;
}
