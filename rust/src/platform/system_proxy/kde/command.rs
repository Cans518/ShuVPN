use crate::connection::model::VpnFailure;
use std::ffi::OsString;
use std::io::{ErrorKind, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 65536;

struct Running {
    child: Child,
    finished: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.finished {
            // The supervisor owns the utility as a grandchild of the host.
            // Terminate its entire group before allowing rollback to run.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
        }
        let _ = self.child.wait();
    }
}

pub(super) fn run(program: &Path, args: &[OsString]) -> Result<String, VpnFailure> {
    execute(program, args, Duration::from_secs(5))
}

fn execute(program: &Path, args: &[OsString], budget: Duration) -> Result<String, VpnFailure> {
    let mut random = [0; 16];
    getrandom::getrandom(&mut random).map_err(|_| error("无法生成 KDE 工具完成标识"))?;
    let marker = format!("__SHUVPN_STATUS_{random:02x?}__:");
    // Dart's exit handler waits for all direct children, including native ones.
    // A supervisor reports the utility's status over its pipe before exiting.
    // "$@" is literal argv forwarding; configuration data is never shell code.
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(r#""$@"; status=$?; printf '\n%s%d\n' "$SHUVPN_STATUS_MARKER" "$status""#)
        .arg("shuvpn-kconfig")
        .arg(program)
        .args(args)
        .env("SHUVPN_STATUS_MARKER", &marker)
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    unsafe {
        command.pre_exec(|| {
            if libc::signal(libc::SIGCHLD, libc::SIG_DFL) == libc::SIG_ERR {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut process = Running {
        child: command
            .spawn()
            .map_err(|_| error("无法启动 KDE 系统配置工具"))?,
        finished: false,
    };
    let output = process
        .child
        .stdout
        .as_mut()
        .ok_or_else(|| error("无法读取 KDE 工具输出"))?;
    let descriptor = output.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(error("无法建立有界 KDE 工具读取"));
    }
    let deadline = Instant::now() + budget;
    let mut bytes = Vec::new();
    let mut eof = false;
    loop {
        let mut chunk = [0; 1024];
        loop {
            let output = process
                .child
                .stdout
                .as_mut()
                .ok_or_else(|| error("KDE 工具输出已关闭"))?;
            match output.read(&mut chunk) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(read) => {
                    if bytes.len() + read > OUTPUT_LIMIT + 256 {
                        return Err(error("KDE 工具输出超过大小预算"));
                    }
                    bytes.extend_from_slice(&chunk[..read]);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(error("读取 KDE 工具输出失败")),
            }
        }
        if eof {
            let text = String::from_utf8(bytes).map_err(|_| error("KDE 配置工具返回了无效文本"))?;
            let (output, trailer) = text
                .strip_suffix('\n')
                .and_then(|text| text.rsplit_once('\n'))
                .ok_or_else(|| error("KDE 工具缺少完成状态"))?;
            let status: u16 = trailer
                .strip_prefix(&marker)
                .and_then(|status| status.parse().ok())
                .filter(|status| *status <= 255)
                .ok_or_else(|| error("KDE 工具完成状态无效"))?;
            process.finished = true;
            if output.len() > OUTPUT_LIMIT {
                return Err(error("KDE 工具输出超过大小预算"));
            }
            if status != 0 {
                return Err(error("KDE 配置工具返回失败"));
            }
            return Ok(output.to_owned());
        }
        if Instant::now() >= deadline {
            return Err(error("KDE 配置操作超时，已终止工具"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn error(message: &str) -> VpnFailure {
    VpnFailure::new("systemProxy", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_exact_output_and_rejects_failure_timeout_and_oversized_output() -> anyhow::Result<()>
    {
        assert_eq!(
            execute(
                Path::new("/bin/printf"),
                &["value\n".into()],
                Duration::from_secs(1)
            )?,
            "value\n"
        );
        let literal = "$(printf unexpected); 'quoted' `printf unwanted` 空值";
        assert_eq!(
            execute(
                Path::new("/bin/printf"),
                &["%s".into(), literal.into()],
                Duration::from_secs(1)
            )?,
            literal
        );
        assert!(execute(Path::new("/bin/false"), &[], Duration::from_secs(1)).is_err());
        let started = Instant::now();
        assert!(execute(
            Path::new("/bin/sleep"),
            &["2".into()],
            Duration::from_millis(30)
        )
        .is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(execute(
            Path::new("/usr/bin/python3"),
            &[
                "-c".into(),
                "import sys; sys.stdout.write('x' * 70000)".into()
            ],
            Duration::from_secs(1)
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn timed_out_utility_cannot_commit_a_late_config_write() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let destination = directory.path().join("late-config");
        assert!(execute(Path::new("/usr/bin/python3"), &[
            "-c".into(),
            "import pathlib,sys,time; time.sleep(0.2); pathlib.Path(sys.argv[1]).write_text('late')".into(),
            destination.as_os_str().to_owned(),
        ], Duration::from_millis(30)).is_err());
        std::thread::sleep(Duration::from_millis(300));
        assert!(!destination.exists());
        Ok(())
    }
}
