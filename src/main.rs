//! tsess — enregistre des sessions terminal (claude, codex, gemini, shell...) et les rouvre.
//! Stockage : ~/.tsess/<nom>.meta (cwd / commande séparée par \t / id de session du harness) + <nom>.log

use std::{env, fs, path::PathBuf, process::{exit, Command}, time::SystemTime};

fn dir() -> PathBuf {
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE")).expect("HOME");
    let d = PathBuf::from(home).join(".tsess");
    fs::create_dir_all(&d).expect("création ~/.tsess");
    d
}

/// Commande de reprise propre à chaque harness ; sinon on relance la commande d'origine.
fn harness(cmd: &[String]) -> Option<&str> {
    cmd.first().map(|c| c.rsplit(['/', '\\']).next().unwrap_or(c).trim_end_matches(".exe"))
}

/// Avec l'id du harness on reprend exactement la bonne session ; sinon la dernière du dossier.
fn resume_cmd(cmd: &[String], id: Option<&str>) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect();
    match (harness(cmd), id) {
        (Some(h @ ("claude" | "codex")), Some(id)) => return tsess::resume_by_id(h, id),
        _ => {}
    }
    match harness(cmd) {
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

fn load(name: &str) -> (String, Vec<String>, Option<String>) {
    let meta = fs::read_to_string(dir().join(format!("{name}.meta"))).unwrap_or_else(|_| {
        eprintln!("session inconnue: {name}");
        exit(1)
    });
    let mut l = meta.lines();
    let cwd = l.next().unwrap_or(".").to_string();
    let cmd = l.next().unwrap_or("").split('\t').filter(|s| !s.is_empty()).map(String::from).collect();
    let id = l.next().filter(|s| !s.is_empty()).map(String::from);
    (cwd, cmd, id)
}

/// Après la sortie du harness : retrouve la session qu'il vient de créer/utiliser dans ce dossier.
fn capture_id(cmd: &[String], cwd: &str, since: SystemTime) -> Option<String> {
    let h = harness(cmd)?;
    tsess::discover(50).into_iter().find(|f| f.harness == h && f.cwd == cwd && f.mtime >= since).map(|f| f.id)
}

fn save(name: &str, cwd: &str, cmd: &[String], id: Option<&str>) {
    fs::write(dir().join(format!("{name}.meta")), format!("{cwd}\n{}\n{}\n", cmd.join("\t"), id.unwrap_or(""))).unwrap();
}

fn ago(t: SystemTime) -> String {
    let m = t.elapsed().map(|d| d.as_secs() / 60).unwrap_or(0);
    match m { 0..=59 => format!("{m} min"), 60..=1439 => format!("{} h", m / 60), _ => format!("{} j", m / 1440) }
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
    let usage = "usage:\n  tsess new [<nom> [-- cmd...]] (sans nom: assistant)\n  tsess open <nom|id>           reprend la session\n  tsess ls\n  tsess log <nom>               affiche l'enregistrement\n  tsess rm <nom>";
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
            save(name, &cwd, &cmd, None);
            let start = SystemTime::now();
            let code = run_recorded(&cwd, &dir().join(format!("{name}.log")), &cmd);
            if let Some(id) = capture_id(&cmd, &cwd, start) { save(name, &cwd, &cmd, Some(&id)) }
            exit(code);
        }
        Some("open") if a.len() == 2 && !dir().join(format!("{}.meta", a[1])).exists() => {
            // pas une session nommée : id (ou début d'id) d'une session Claude/Codex
            let Some(f) = tsess::discover(500).into_iter().find(|f| a[1].len() >= 6 && f.id.starts_with(&a[1])) else {
                eprintln!("session inconnue: {}", a[1]); exit(1)
            };
            let r = f.resume();
            eprintln!("↻ {} dans {}", r.join(" "), f.cwd);
            exit(run_recorded(&f.cwd, &dir().join(format!("{}.log", f.id)), &r));
        }
        Some("open") if a.len() == 2 => {
            let (cwd, cmd, id) = load(&a[1]);
            let r = resume_cmd(&cmd, id.as_deref());
            eprintln!("↻ {} dans {cwd}", r.join(" "));
            let start = SystemTime::now();
            let code = run_recorded(&cwd, &dir().join(format!("{}.log", a[1])), &r);
            // codex/claude peuvent créer un nouvel id à la reprise : on garde le plus récent
            if let Some(nid) = capture_id(&cmd, &cwd, start) { save(&a[1], &cwd, &cmd, Some(&nid)) }
            exit(code);
        }
        Some("ls") => {
            let mut v: Vec<_> = fs::read_dir(dir()).unwrap().flatten()
                .filter_map(|e| e.file_name().to_str()?.strip_suffix(".meta").map(String::from)).collect();
            v.sort();
            if !v.is_empty() { println!("Sessions nommées :") }
            for n in v { let (cwd, cmd, _) = load(&n); println!("  {n:20} {:24} {cwd}", cmd.join(" ")) }
            println!("Récentes (tsess open <id>) :");
            for f in tsess::discover(20) {
                println!("  {:8} {:6} {:6} {:50} {}", &f.id[..8.min(f.id.len())], f.harness, ago(f.mtime), f.title.chars().take(50).collect::<String>(), f.cwd);
            }
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
        assert_eq!(resume_cmd(&v(&["/usr/bin/claude", "-p"]), None), v(&["claude", "--continue"]));
        assert_eq!(resume_cmd(&v(&["claude"]), Some("abc")), v(&["claude", "--resume", "abc"]));
        assert_eq!(resume_cmd(&v(&["codex"]), None), v(&["codex", "resume", "--last"]));
        assert_eq!(resume_cmd(&v(&["zsh"]), Some("x")), v(&["zsh"]));
        assert!(valid_name("audit-1") && !valid_name("../x") && !valid_name(""));
    }
}
