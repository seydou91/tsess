//! Découverte des sessions stockées par les harness eux-mêmes (Claude Code, Codex).

use std::{env, fs, io::{BufRead, BufReader}, path::{Path, PathBuf}, time::SystemTime};
use serde_json::Value;

pub fn home() -> PathBuf {
    PathBuf::from(env::var("HOME").or_else(|_| env::var("USERPROFILE")).unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub harness: &'static str,
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub mtime: SystemTime,
}

impl Found {
    pub fn resume(&self) -> Vec<String> {
        resume_by_id(self.harness, &self.id)
    }
}

pub fn resume_by_id(harness: &str, id: &str) -> Vec<String> {
    match harness {
        "codex" => vec!["codex".into(), "resume".into(), id.into()],
        _ => vec!["claude".into(), "--resume".into(), id.into()],
    }
}

/// Tous les .jsonl sous `root` (récursif), avec leur date de modif.
fn jsonl_files(root: &Path, out: &mut Vec<(PathBuf, SystemTime)>) {
    for e in fs::read_dir(root).into_iter().flatten().flatten() {
        let p = e.path();
        // les sous-agents de Claude ne se reprennent pas seuls
        if p.is_dir() { if !p.ends_with("subagents") { jsonl_files(&p, out) } }
        else if p.extension().is_some_and(|x| x == "jsonl")
            && !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("agent-")) {
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) { out.push((p, t)) }
        }
    }
}

/// Premier message utilisateur « humain » (on saute les balises système `<...>`).
fn human_text(v: &Value) -> Option<String> {
    let t = match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().find_map(|c| c.get("text")?.as_str().map(String::from))?,
        _ => return None,
    };
    let t = t.trim();
    (!t.is_empty() && !t.starts_with('<')).then(|| t.chars().take(80).collect::<String>().replace('\n', " "))
}

fn parse(harness: &'static str, path: &Path, mtime: SystemTime) -> Option<Found> {
    let (mut id, mut cwd, mut title) = (None, None, None);
    // ponytail: on ne lit que les 300 premières lignes, suffisant pour cwd + 1er message
    for line in BufReader::new(fs::File::open(path).ok()?).lines().take(300).map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if harness == "claude" {
            if cwd.is_none() { cwd = v.get("cwd").and_then(|c| c.as_str()).map(String::from) }
            if title.is_none() && v.get("type").and_then(|t| t.as_str()) == Some("user") {
                title = v.pointer("/message/content").and_then(human_text);
            }
        } else {
            let p = v.get("payload");
            // seule la 1re session_meta compte (les suivantes décrivent la session parente d'un fork)
            if id.is_none() && v.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
                id = p.and_then(|p| p.get("id")?.as_str().map(String::from));
                cwd = p.and_then(|p| p.get("cwd")?.as_str().map(String::from));
            }
            if title.is_none() && p.and_then(|p| p.get("role")?.as_str()) == Some("user") {
                title = p.and_then(|p| p.get("content")).and_then(human_text);
            }
        }
        if cwd.is_some() && title.is_some() && (harness == "claude" || id.is_some()) { break }
    }
    if harness == "claude" { id = Some(path.file_stem()?.to_str()?.to_string()) }
    Some(Found { harness, id: id?, cwd: cwd?, title: title.unwrap_or_else(|| "(sans titre)".into()), mtime })
}

/// Les `limit` sessions les plus récentes de tous les harness connus.
pub fn discover(limit: usize) -> Vec<Found> {
    let h = home();
    let mut all = Vec::new();
    for (harness, root) in [("claude", h.join(".claude/projects")), ("codex", h.join(".codex/sessions"))] {
        let mut files = Vec::new();
        jsonl_files(&root, &mut files);
        files.sort_by(|a, b| b.1.cmp(&a.1));
        all.extend(files.into_iter().take(limit).filter_map(|(p, t)| parse(harness, &p, t)));
    }
    all.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    let mut seen = std::collections::HashSet::new();
    all.retain(|f| seen.insert(f.id.clone()));
    all.truncate(limit);
    all
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn parses_both_formats() {
        let d = env::temp_dir().join(format!("tsess-test-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let c = d.join("abc-123.jsonl");
        fs::write(&c, concat!(
            r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"<cmd>x</cmd>"}}"#, "\n",
            r#"{"type":"user","cwd":"/p","message":{"role":"user","content":"corrige le bug"}}"#, "\n")).unwrap();
        let f = parse("claude", &c, SystemTime::now()).unwrap();
        assert_eq!((f.id.as_str(), f.cwd.as_str(), f.title.as_str()), ("abc-123", "/p", "corrige le bug"));

        let x = d.join("rollout.jsonl");
        fs::write(&x, concat!(
            r#"{"type":"session_meta","payload":{"id":"cx-9","cwd":"/q"}}"#, "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<env/>"}]}}"#, "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"ajoute des tests"}]}}"#, "\n")).unwrap();
        let f = parse("codex", &x, SystemTime::now()).unwrap();
        assert_eq!((f.id.as_str(), f.cwd.as_str(), f.title.as_str()), ("cx-9", "/q", "ajoute des tests"));
        assert_eq!(f.resume(), vec!["codex", "resume", "cx-9"]);
        fs::remove_dir_all(d).ok();
    }
}
