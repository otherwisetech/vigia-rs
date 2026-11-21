//! Talking to the operating system: notifications and launch at login.
use std::path::{Path, PathBuf};

/// AppleScript string literal.
pub fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// System notification (macOS Notification Center, Linux notify-send, Windows toast through
/// PowerShell). Fire and forget.
pub fn notify(title: &str, body: &str) {
    let (prog, args): (&str, Vec<String>) = if cfg!(target_os = "macos") {
        ("osascript", vec!["-e".into(), format!("display notification {} with title {} sound name \"default\"", applescript_string(body), applescript_string(title))]) // i18n-ok: AppleScript
    } else if cfg!(windows) {
        let ps = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); $x = $t.GetElementsByTagName('text'); $x.Item(0).AppendChild($t.CreateTextNode('{}')) > $null; $x.Item(1).AppendChild($t.CreateTextNode('{}')) > $null; [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Vigia').Show([Windows.UI.Notifications.ToastNotification]::new($t))", // i18n-ok: PowerShell
            title.replace('\'', "''"),
            body.replace('\'', "''")
        );
        ("powershell", vec!["-NoProfile".into(), "-Command".into(), ps])
    } else {
        ("notify-send", vec!["--app-name=Vigía".into(), title.to_string(), body.to_string()]) // i18n-ok: app name
    };
    std::thread::spawn(move || {
        let _ = std::process::Command::new(prog).args(&args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    });
}

pub const LAUNCH_LABEL: &str = "com.otherwisetech.vigia";

/// The `.app` bundle this binary runs from, if any.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// LaunchAgent that starts Vigía in the background at login (through `open` when it's a
/// bundle, so LaunchServices never starts a second copy).
pub fn launch_agent_plist(exe: &Path) -> String {
    let args: Vec<String> = match bundle_of(exe) {
        Some(app) => vec!["/usr/bin/open".into(), "-g".into(), "-a".into(), app.display().to_string(), "--args".into(), "--background".into()],
        None => vec![exe.display().to_string(), "--background".into()],
    };
    let items: String = args.iter().map(|a| format!("    <string>{}</string>\n", xml(a))).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>{LAUNCH_LABEL}</string>\n  <key>ProgramArguments</key>\n  <array>\n{items}  </array>\n  <key>RunAtLoad</key><true/>\n  <key>ProcessType</key><string>Interactive</string>\n</dict>\n</plist>\n" // i18n-ok: plist
    )
}

/// XDG autostart entry.
pub fn autostart_desktop(exe: &Path) -> String {
    format!("[Desktop Entry]\nType=Application\nName=Vigía\nExec=\"{}\" --background\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n", exe.display()) // i18n-ok: desktop entry
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

fn login_file() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/LaunchAgents").join(format!("{LAUNCH_LABEL}.plist")))
    } else if cfg!(windows) {
        None
    } else {
        std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| home().map(|h| h.join(".config"))).map(|d| d.join("autostart/vigia.desktop"))
    }
}

pub fn launch_at_login() -> bool {
    if cfg!(windows) {
        return std::process::Command::new("reg").args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Vigia"]).output().is_ok_and(|o| o.status.success());
    }
    login_file().is_some_and(|f| f.exists())
}

pub fn set_launch_at_login(on: bool) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    if cfg!(windows) {
        let run = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let status = if on {
            std::process::Command::new("reg").args(["add", run, "/v", "Vigia", "/t", "REG_SZ", "/d", &format!("\"{}\" --background", exe.display()), "/f"]).status()?
        } else {
            std::process::Command::new("reg").args(["delete", run, "/v", "Vigia", "/f"]).status()?
        };
        return if status.success() || !on { Ok(()) } else { Err(std::io::Error::other(status.to_string())) };
    }
    let Some(f) = login_file() else { return Ok(()) };
    if !on {
        let _ = std::fs::remove_file(&f);
        return Ok(());
    }
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&f, if cfg!(target_os = "macos") { launch_agent_plist(&exe) } else { autostart_desktop(&exe) })
}

#[cfg(test)]
mod tests {
    use super::{autostart_desktop, bundle_of, launch_agent_plist};
    use std::path::Path;

    #[test]
    fn login_items() {
        let exe = Path::new("/Applications/Vigía.app/Contents/MacOS/vigia-app");
        assert_eq!(bundle_of(exe).as_deref(), Some(Path::new("/Applications/Vigía.app")));
        let p = launch_agent_plist(exe);
        assert!(p.contains("<string>/usr/bin/open</string>") && p.contains("<string>/Applications/Vigía.app</string>") && p.contains("<string>--background</string>"));
        let p = launch_agent_plist(Path::new("/home/a/bin/vigia-app"));
        assert!(p.contains("<string>/home/a/bin/vigia-app</string>") && !p.contains("open"));
        assert!(autostart_desktop(Path::new("/usr/bin/vigia-app")).contains("Exec=\"/usr/bin/vigia-app\" --background"));
    }
}
