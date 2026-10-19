use crate::error::{CommandError, CommandResult};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Colors {
    pub accent: String,
    pub canvas: String,
    pub surface: String,
    pub ink: String,
    pub muted: String,
    pub border: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomTheme {
    pub name: String,
    pub light: Colors,
    pub dark: Colors,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub palette: String,
    pub custom_theme: CustomTheme,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            palette: "ocean".into(),
            custom_theme: CustomTheme {
                name: "我的主题".into(),
                light: Colors {
                    accent: "#2864ad".into(),
                    canvas: "#f3f6fb".into(),
                    surface: "#ffffff".into(),
                    ink: "#25364b".into(),
                    muted: "#5c6b80".into(),
                    border: "#dce4ef".into(),
                },
                dark: Colors {
                    accent: "#99c2ff".into(),
                    canvas: "#141c29".into(),
                    surface: "#1c2838".into(),
                    ink: "#e5edf8".into(),
                    muted: "#a6b7cd".into(),
                    border: "#35465d".into(),
                },
            },
        }
    }
}

impl Appearance {
    fn validate(&self) -> CommandResult<()> {
        if !["ocean", "jade", "iris", "amber", "nailong", "custom"].contains(&self.palette.as_str())
        {
            return Err(CommandError::new("theme", "请选择有效的配色方案"));
        }
        if self.custom_theme.name.trim().is_empty() || self.custom_theme.name.chars().count() > 32 {
            return Err(CommandError::new("theme", "主题名称需要 1 至 32 个字符"));
        }
        for colors in [&self.custom_theme.light, &self.custom_theme.dark] {
            for color in [
                &colors.accent,
                &colors.canvas,
                &colors.surface,
                &colors.ink,
                &colors.muted,
                &colors.border,
            ] {
                if color.len() != 7
                    || !color.starts_with('#')
                    || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                {
                    return Err(CommandError::new("theme", "主题颜色必须使用 #RRGGBB 格式"));
                }
            }
        }
        Ok(())
    }
}

pub struct AppearanceStore {
    path: PathBuf,
    current: Mutex<Appearance>,
}

impl AppearanceStore {
    pub fn open(path: PathBuf) -> CommandResult<Self> {
        let current = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Appearance>(&bytes)
                .map_err(|error| CommandError::new("theme", &format!("主题文件无效：{error}")))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Appearance::default(),
            Err(error) => return Err(CommandError::new("theme", &error.to_string())),
        };
        current.validate()?;
        Ok(Self {
            path,
            current: Mutex::new(current),
        })
    }

    pub async fn snapshot(&self) -> Appearance {
        self.current.lock().await.clone()
    }

    pub async fn save(&self, appearance: Appearance) -> CommandResult<Appearance> {
        appearance.validate()?;
        let mut current = self.current.lock().await;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| CommandError::new("theme", "主题目录无效"))?;
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut file, &appearance)?;
            file.write_all(b"\n")?;
            file.as_file().sync_all()?;
            file.persist(&self.path)?;
            Ok(())
        };
        write().map_err(|error| CommandError::new("theme", &format!("主题保存失败：{error}")))?;
        *current = appearance.clone();
        Ok(appearance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persists_custom_theme_and_preserves_it_after_invalid_input(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("appearance.json");
        let store = AppearanceStore::open(path.clone())?;
        let mut custom = Appearance {
            palette: "custom".into(),
            ..Appearance::default()
        };
        custom.custom_theme.light.accent = "#8a456c".into();
        store.save(custom.clone()).await?;
        custom.custom_theme.light.accent = "url(remote)".into();
        assert!(store.save(custom).await.is_err());
        let restored = AppearanceStore::open(path)?.snapshot().await;
        assert_eq!(restored.palette, "custom");
        assert_eq!(restored.custom_theme.light.accent, "#8a456c");
        Ok(())
    }
}
