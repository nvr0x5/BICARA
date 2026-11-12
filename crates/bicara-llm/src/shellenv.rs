//! Apps started from Finder / the Start menu don't inherit the user's shell PATH or env.
//! We read them once from a login shell so `claude` / `codex` run exactly like in a terminal.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

static ENV: OnceLock<HashMap<String, String>> = OnceLock::new();

/// The user's login-shell environment (falls back to the current process env).
pub fn user_env() -> &'static HashMap<String, String> {
    ENV.get_or_init(|| {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
            let out = std::process::Command::new(&shell)
                .args(["-l", "-i", "-c", "env -0"])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();
            if let Ok(o) = out {
                for kv in o.stdout.split(|b| *b == 0) {
                    let s = String::from_utf8_lossy(kv);
                    if let Some((k, v)) = s.split_once('=') {
                        if !k.is_empty() && !k.contains('\n') {
                            env.insert(k.to_string(), v.to_string());
                        }
                    }
                }
            }
        }
        env
    })
}

/// Locate a CLI: explicit path → login-shell PATH → common install locations.
pub fn find_binary(name: &str, explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit.filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let exe = if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_string()
    };
    let exe_alt = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path = user_env()
        .get("PATH")
        .or_else(|| user_env().get("Path"))
        .cloned()
        .unwrap_or_default();
    let mut dirs: Vec<PathBuf> = path.split(sep).map(PathBuf::from).collect();
    if let Some(home) = user_env()
        .get("HOME")
        .or_else(|| user_env().get("USERPROFILE"))
    {
        let h = PathBuf::from(home);
        dirs.extend([
            h.join(".local/bin"),
            h.join(".claude/local"),
            h.join(".npm-global/bin"),
            h.join("AppData/Roaming/npm"),
        ]);
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.into_iter()
        .flat_map(|d| [d.join(&exe), d.join(&exe_alt)])
        .find(|p| p.is_file())
}
