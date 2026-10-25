#![cfg_attr(windows, windows_subsystem = "windows")]
//! tsess-bar — icône de barre de menus : sessions nommées + récentes Claude/Codex, clic = reprise dans un terminal.

use std::{env, fs, path::PathBuf, process::Command, time::{Duration, Instant}};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

type State = (Vec<(String, String)>, Vec<(String, String)>);

/// (sessions nommées, sessions récentes Claude/Codex) sous forme (id de menu, libellé).
fn sessions() -> State {
    let recent = tsess::discover(15).into_iter().map(|f| {
        let folder = f.cwd.rsplit(['/', '\\']).next().unwrap_or("").to_string();
        let title: String = f.title.chars().take(45).collect();
        (format!("s:{}", f.id), format!("[{}] {folder} — {title}", f.harness))
    }).collect();
    (named(), recent)
}

fn named() -> Vec<(String, String)> {
    let d = tsess::home().join(".tsess");
    let mut v: Vec<_> = fs::read_dir(&d).into_iter().flatten().flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.strip_suffix(".meta")?.to_string();
            let meta = fs::read_to_string(e.path()).ok()?;
            let cmd = meta.lines().nth(1).unwrap_or("").replace('\t', " ");
            Some((name, cmd))
        })
        .collect();
    v.sort();
    v
}

fn build_menu((list, recent): &State) -> Menu {
    let m = Menu::new();
    if list.is_empty() {
        m.append(&MenuItem::new("Aucune session nommée", false, None)).unwrap();
    }
    for (name, cmd) in list {
        // l'id du menu = nom de session (déjà validé par tsess : [A-Za-z0-9-_.])
        m.append(&MenuItem::with_id(name.as_str(), format!("{name}  —  {cmd}"), true, None)).unwrap();
    }
    m.append(&PredefinedMenuItem::separator()).unwrap();
    m.append(&MenuItem::new("Récentes (Claude, Codex)", false, None)).unwrap();
    for (id, label) in recent {
        m.append(&MenuItem::with_id(id.as_str(), label, true, None)).unwrap();
    }
    m.append(&PredefinedMenuItem::separator()).unwrap();
    m.append(&MenuItem::with_id("__new", "Nouvelle session…", true, None)).unwrap();
    m.append(&CheckMenuItem::with_id("__login", "Lancer au démarrage", true, login_enabled(), None)).unwrap();
    m.append(&MenuItem::with_id("__quit", "Quitter", true, None)).unwrap();
    m
}

fn tsess_bin() -> String {
    let exe = if cfg!(windows) { "tsess.exe" } else { "tsess" };
    env::current_exe().ok().and_then(|p| Some(p.parent()?.join(exe)))
        .filter(|p| p.exists()).map(|p| p.display().to_string()).unwrap_or("tsess".into())
}

/// Ouvre un nouveau terminal qui exécute `tsess <args>`.
fn run_in_terminal(args: &[&str]) {
    let t = tsess_bin();
    if cfg!(target_os = "macos") {
        let line = format!("'{t}' {}", args.join(" "));
        osascript(format!("tell application \"Terminal\"\ndo script \"{line}\"\nactivate\nend tell"));
    } else if cfg!(windows) {
        let _ = Command::new("cmd").args(["/c", "start", "tsess", &t]).args(args).spawn();
    } else {
        // premier émulateur de terminal disponible
        for (term, flag) in [("x-terminal-emulator", "-e"), ("gnome-terminal", "--"), ("konsole", "-e"), ("xfce4-terminal", "-x"), ("xterm", "-e")] {
            if Command::new(term).arg(flag).arg(&t).args(args).spawn().is_ok() { return }
        }
    }
}

fn osascript(script: String) {
    let _ = Command::new("osascript").arg("-e").arg(script).spawn();
}

/// macOS : boîtes de dialogue natives. Ailleurs : assistant `tsess new` dans un terminal.
fn new_session() {
    if !cfg!(target_os = "macos") { return run_in_terminal(&["new"]) }
    let t = tsess_bin();
    osascript(format!(r#"
set n to text returned of (display dialog "Nom de la session" default answer "" with title "tsess")
set c to text returned of (display dialog "Commande (claude, codex, gemini, zsh…)" default answer "claude" with title "tsess")
set f to POSIX path of (choose folder with prompt "Dossier de travail")
tell application "Terminal"
do script "cd " & quoted form of f & " && '{t}' new " & quoted form of n & " -- " & c
activate
end tell"#));
}

fn home() -> PathBuf {
    PathBuf::from(env::var("HOME").or_else(|_| env::var("USERPROFILE")).unwrap_or_default())
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Fichier de démarrage auto (macOS LaunchAgent / Linux XDG autostart). Windows : clé registre Run.
fn agent_path() -> PathBuf {
    if cfg!(target_os = "macos") { home().join("Library/LaunchAgents/com.tsess.bar.plist") }
    else { home().join(".config/autostart/tsess-bar.desktop") }
}

fn login_enabled() -> bool {
    if cfg!(windows) {
        return Command::new("reg").args(["query", RUN_KEY, "/v", "tsess-bar"]).output().map(|o| o.status.success()).unwrap_or(false);
    }
    agent_path().exists()
}

fn toggle_login() {
    let exe = env::current_exe().map(|e| e.display().to_string()).unwrap_or_default();
    let on = login_enabled();
    if cfg!(windows) {
        let a: Vec<&str> = if on { vec!["delete", RUN_KEY, "/v", "tsess-bar", "/f"] }
            else { vec!["add", RUN_KEY, "/v", "tsess-bar", "/d", &exe, "/f"] };
        let _ = Command::new("reg").args(a).status();
        return;
    }
    let p = agent_path();
    if on { let _ = fs::remove_file(&p); return }
    let content = if cfg!(target_os = "macos") { format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.tsess.bar</string>
<key>ProgramArguments</key><array><string>{exe}</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>
"#) } else { format!("[Desktop Entry]\nType=Application\nName=tsess\nExec=\"{exe}\"\n") };
    let _ = fs::create_dir_all(p.parent().unwrap());
    let _ = fs::write(&p, content);
}

/// Icône 32×32 dessinée en code : cadre de terminal clair + point rouge « enregistrement ».
fn icon() -> Icon {
    const N: i32 = 32;
    let mut px = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let i = ((y * N + x) * 4) as usize;
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let d = (dx * dx + dy * dy).sqrt();
            let c: Option<[u8; 4]> = if d <= 9.5 { Some([235, 45, 45, 255]) }      // point rouge
                else if (12.0..=15.5).contains(&d) { Some([255, 255, 255, 255]) } // anneau blanc
                else { None };
            if let Some(c) = c { px[i..i + 4].copy_from_slice(&c) }
        }
    }
    Icon::from_rgba(px, N as u32, N as u32).unwrap()
}

fn main() {
    let event_loop = EventLoopBuilder::new().build();
    let mut tray = None;
    let mut last: State = Default::default();
    let menu_rx = MenuEvent::receiver();

    event_loop.run(move |event, _, flow| {
        // ponytail: relit ~/.tsess et les historiques toutes les 2 s, passer à un watcher FS si ça gêne
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(2));

        if let Event::NewEvents(StartCause::Init) = event {
            last = sessions();
            tray = Some(TrayIconBuilder::new().with_icon(icon()).with_tooltip("tsess")
                .with_menu(Box::new(build_menu(&last))).build().unwrap());
        }
        if let Some(t) = &tray {
            let now = sessions();
            if now != last { t.set_menu(Some(Box::new(build_menu(&now)))); last = now; }
        }
        while let Ok(ev) = menu_rx.try_recv() {
            match ev.id.0.as_str() {
                "__quit" => *flow = ControlFlow::Exit,
                "__new" => new_session(),
                "__login" => toggle_login(),
                id => run_in_terminal(&["open", id.trim_start_matches("s:")]),
            }
        }
    });
}
