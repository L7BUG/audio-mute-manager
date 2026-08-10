//! 配置持久化:勾选的应用(按进程名)存到文本文件,重启后自动恢复。
//!
//! 格式:每行一个进程名,`#` 开头为注释,忽略空行。
//! 不引入 serde/toml 依赖,保持 exe 体积。纯 std,平台无关,可单测。

use std::fs;
use std::path::Path;

/// 配置文件内容:被管理的进程名集合
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    managed: Vec<String>,
}

impl Config {
    /// 从文件加载;不存在或损坏时返回空配置(不 panic)
    pub fn load(path: &Path) -> Config {
        let Ok(text) = fs::read_to_string(path) else {
            return Config::default();
        };
        let mut managed = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            managed.push(line.to_string());
        }
        Config { managed }
    }

    /// 保存到文件(自动创建父目录);失败静默忽略,不破坏主流程
    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let mut text = String::from("# audio-mute-manager 被管理应用(进程名),每行一个\n");
        for name in &self.managed {
            text.push_str(name);
            text.push('\n');
        }
        let _ = fs::write(path, text);
    }

    /// 被管理的进程名列表
    pub fn managed(&self) -> &[String] {
        &self.managed
    }

    /// 替换整个集合
    pub fn set_managed(&mut self, names: Vec<String>) {
        self.managed = names;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("amm-config-{}-{}", tag, std::process::id()))
    }

    #[test]
    fn load_missing_returns_empty() {
        let c = Config::load(&tmp_path("missing"));
        assert!(c.managed().is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let path = tmp_path("roundtrip");
        let mut c = Config::default();
        c.set_managed(vec!["chrome.exe".into(), "wechat.exe".into()]);
        c.save(&path);
        let loaded = Config::load(&path);
        assert_eq!(loaded.managed(), &["chrome.exe", "wechat.exe"]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn load_ignores_comments_and_blanks() {
        let path = tmp_path("comments");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# 注释\n\nchrome.exe\n  \n").unwrap();
        let c = Config::load(&path);
        assert_eq!(c.managed(), &["chrome.exe"]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn set_managed_replaces() {
        let mut c = Config::default();
        c.set_managed(vec!["a.exe".into()]);
        c.set_managed(vec!["b.exe".into(), "c.exe".into()]);
        assert_eq!(c.managed(), &["b.exe", "c.exe"]);
    }
}
