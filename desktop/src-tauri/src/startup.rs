use crate::error::{CommandError, CommandResult};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
mod windows {
    use std::{io, path::Path};
    use windows_registry::{Transaction, CURRENT_USER};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    fn optional<T>(result: windows_registry::Result<T>) -> io::Result<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            // windows-result preserves HRESULTs rather than Win32 error numbers.
            Err(error) if [0x80070002u32, 0x80070003].contains(&(error.code().0 as u32)) => {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn command(path: &Path) -> io::Result<String> {
        let path = path
            .to_str()
            .ok_or_else(|| io::Error::other("启动路径无法编码"))?;
        Ok(format!("\"{path}\" --hidden"))
    }

    pub fn enabled(name: &str) -> io::Result<bool> {
        if optional(CURRENT_USER.open(RUN).and_then(|key| key.get_string(name)))?.is_none() {
            return Ok(false);
        }
        let approved = optional(
            CURRENT_USER
                .open(APPROVED)
                .and_then(|key| key.get_value(name)),
        )?;
        Ok(approved.is_none_or(|value| value.first().is_none_or(|state| state & 1 == 0)))
    }

    pub fn set(name: &str, path: &Path, enabled: bool) -> io::Result<()> {
        let transaction = Transaction::new()?;
        if enabled {
            let run = CURRENT_USER
                .options()
                .read()
                .write()
                .create()
                .transaction(&transaction)
                .open(RUN)?;
            let approved = CURRENT_USER
                .options()
                .read()
                .write()
                .create()
                .transaction(&transaction)
                .open(APPROVED)?;
            run.set_string(name, command(path)?)?;
            // Respect Task Manager's startup control, and clear its disabled mark
            // only when the user explicitly enables startup in ShuVPN.
            approved.set_bytes(
                name,
                windows_registry::Type::Bytes,
                &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )?;
        } else {
            for path in [RUN, APPROVED] {
                if let Some(key) = optional(
                    CURRENT_USER
                        .options()
                        .write()
                        .transaction(&transaction)
                        .open(path),
                )? {
                    optional(key.remove_value(name))?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn current_user_entry_quotes_spaced_paths_and_can_be_removed(
        ) -> Result<(), Box<dyn std::error::Error>> {
            let name = format!("ShuVPN.Autostart.Test.{}", std::process::id());
            struct Cleanup(String);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    let _ = set(&self.0, Path::new("unused"), false);
                }
            }
            let _cleanup = Cleanup(name.clone());
            let path = Path::new(r"C:\Users\Example User\ShuVPN\shuvpn.exe");
            set(&name, path, true)?;
            assert!(enabled(&name)?);
            assert_eq!(
                CURRENT_USER.open(RUN)?.get_string(&name)?,
                r#""C:\Users\Example User\ShuVPN\shuvpn.exe" --hidden"#
            );
            set(&name, path, false)?;
            assert!(!enabled(&name)?);
            Ok(())
        }
    }
}

pub fn enabled() -> CommandResult<bool> {
    #[cfg(windows)]
    {
        windows::enabled("ShuVPN").map_err(|error| CommandError::new("startup", &error.to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        linux::enabled().map_err(|error| CommandError::new("startup", &error.to_string()))
    }
}

pub fn set(enabled: bool) -> CommandResult<()> {
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()
            .map_err(|error| CommandError::new("startup", &error.to_string()))?;
        windows::set("ShuVPN", &executable, enabled)
            .map_err(|error| CommandError::new("startup", &error.to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        linux::set(enabled).map_err(|error| CommandError::new("startup", &error.to_string()))
    }
}
