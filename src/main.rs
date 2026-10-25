//! tsess — enregistre des sessions terminal (claude, codex, gemini, shell...) et les rouvre.
//! Stockage : ~/.tsess/<nom>.meta (ligne 1 = cwd, ligne 2 = commande séparée par \t) + <nom>.log

use std::{env, fs, path::PathBuf, process::{exit, Command}};

fn dir() -> PathBuf {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).expect("HOME");
    let d = PathBuf::from(home).join(".tsess");
    fs::create_dir_all(&d).expect("création ~/.tsess");
    d
}

/// Commande de reprise propre à chaque harness ; sinon on relance la commande d'origine.
fn resume_cmd(cmd: &[String]) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect();
    match cmd.first().map(|c| c.rsplit('/').next().unwrap_or(c)) {
        Some("claude") => s(&["claude", "--continue"]),
        Some("codex") => s(&["codex", "resume", "--last"]),
        Some("gemini") => s(&["gemini", "--resume", "latest"]),
        Some("opencode") => s(&["opencode", "--continue"]),
        _ => cmd.to_vec(),
    }
}

/// Lance `cmd` dans `cwd` en enregistrant tout le terminal dans `log` (via script(1)).
fn run_recorded(cwd: &str, log: &PathBuf, cmd: &[String]) -> i32 {
    let mut c = Command::new(if cfg!(windows) { &cmd[0] } else { "script" });
    if cfg!(windows) {
        // ponytail: pas de script(1) sous Windows → pas de log, la reprise passe par le harness
        let _ = log;
        c.args(&cmd[1..]);
    } else if cfg!(target_os = "macos") {
        c.arg("-q").arg("-a").arg(log).args(cmd);
    } else {
        // script Linux : -c attend une seule chaîne shell
        let line = cmd.iter().map(|a| format!("'{}'", a.replace('\'', "'\\''"))).collect::<Vec<_>>().join(" ");
        c.args(["-q", "-a", "-c", &line]).arg(log);
    }
    c.current_dir(cwd).status().map(|s| s.code().unwrap_or(1)).unwrap_or_else(|e| {
        eprintln!("échec lancement: {e}");
        1
    })
}

fn load(name: &str) -> (String, Vec<String>) {
    let meta = fs::read_to_string(dir().join(format!("{name}.meta"))).unwrap_or_else(|_| {
        eprintln!("session inconnue: {name}");
        exit(1)
    });
    let mut l = meta.lines();
    let cwd = l.next().unwrap_or(".").to_string();
    let cmd = l.next().unwrap_or("").split('\t').filter(|s| !s.is_empty()).map(String::from).collect();
    (cwd, cmd)
}

/// Question dans le terminal ; renvoie `default` si réponse vide.
fn ask(q: &str, default: &str) -> String {
    use std::io::Write;
    print!("{q} [{default}]: ");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    let s = s.trim();
    if s.is_empty() { default.into() } else { s.into() }
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && !n.starts_with('.') && n.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c))
}

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    let usage = "usage:\n  tsess new [<nom> [-- cmd...]] (sans nom: assistant)\n  tsess open <nom>              reprend la session\n  tsess ls\n  tsess log <nom>               affiche l'enregistrement\n  tsess rm <nom>";
    match a.first().map(String::as_str) {
        Some("new") if a.len() == 1 => {
            // assistant interactif (utilisé par tsess-bar sous Linux/Windows)
            let name = ask("Nom de la session", "");
            let cmd = ask("Commande", "claude");
            let cwd = ask("Dossier", &env::current_dir().unwrap().display().to_string());
            if env::set_current_dir(&cwd).is_err() { eprintln!("dossier introuvable: {cwd}"); exit(1) }
            let mut args = vec!["new".to_string(), name, "--".into()];
            args.extend(cmd.split_whitespace().map(String::from));
            exit(Command::new(env::current_exe().unwrap()).args(args).status().map(|s| s.code().unwrap_or(1)).unwrap_or(1));
        }
        Some("new") if a.len() >= 2 => {
            let name = &a[1];
            if !valid_name(name) { eprintln!("nom invalide: {name}"); exit(1) }
            let mut cmd: Vec<String> = a.iter().skip(2).skip_while(|x| *x == "--").cloned().collect();
            if cmd.is_empty() { cmd.push(env::var("SHELL").unwrap_or(if cfg!(windows) { "powershell".into() } else { "/bin/sh".into() })) }
            let cwd = env::current_dir().unwrap().display().to_string();
            fs::write(dir().join(format!("{name}.meta")), format!("{cwd}\n{}\n", cmd.join("\t"))).unwrap();
            exit(run_recorded(&cwd, &dir().join(format!("{name}.log")), &cmd));
        }
        Some("open") if a.len() == 2 => {
            let (cwd, cmd) = load(&a[1]);
            let r = resume_cmd(&cmd);
            eprintln!("↻ {} dans {cwd}", r.join(" "));
            exit(run_recorded(&cwd, &dir().join(format!("{}.log", a[1])), &r));
        }
        Some("ls") => {
            let mut v: Vec<_> = fs::read_dir(dir()).unwrap().flatten()
                .filter_map(|e| e.file_name().to_str()?.strip_suffix(".meta").map(String::from)).collect();
            v.sort();
            for n in v { let (cwd, cmd) = load(&n); println!("{n:20} {:30} {cwd}", cmd.join(" ")) }
        }
        Some("log") if a.len() == 2 => {
            let _ = load(&a[1]);
            let st = Command::new("less").arg("-R").arg(dir().join(format!("{}.log", a[1]))).status();
            exit(st.map(|s| s.code().unwrap_or(0)).unwrap_or(1));
        }
        Some("rm") if a.len() == 2 => {
            let _ = load(&a[1]);
            for ext in ["meta", "log"] { let _ = fs::remove_file(dir().join(format!("{}.{ext}", a[1]))); }
        }
        _ => { eprintln!("{usage}"); exit(2) }
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn resume_and_names() {
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(resume_cmd(&v(&["/usr/bin/claude", "-p"])), v(&["claude", "--continue"]));
        assert_eq!(resume_cmd(&v(&["codex"])), v(&["codex", "resume", "--last"]));
        assert_eq!(resume_cmd(&v(&["zsh"])), v(&["zsh"]));
        assert!(valid_name("audit-1") && !valid_name("../x") && !valid_name(""));
    }
}
