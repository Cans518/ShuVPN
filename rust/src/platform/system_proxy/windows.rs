use super::{failure, Settings};
use crate::connection::model::VpnFailure;
use std::ffi::c_void;
use std::ptr::null_mut;

#[repr(C)]
union Value {
    number: u32,
    string: *mut u16,
    file_time: [u32; 2],
}

#[repr(C)]
struct OptionValue {
    option: u32,
    value: Value,
}

#[repr(C)]
struct Options {
    size: u32,
    connection: *mut u16,
    count: u32,
    error: u32,
    options: *mut OptionValue,
}

#[link(name = "wininet")]
extern "system" {
    fn InternetQueryOptionW(
        handle: *mut c_void,
        option: u32,
        buffer: *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn InternetSetOptionW(handle: *mut c_void, option: u32, buffer: *mut c_void, size: u32) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalFree(memory: *mut c_void) -> *mut c_void;
}

const PER_CONNECTION: u32 = 75;
const FLAGS: u32 = 1;
const SERVER: u32 = 2;
const BYPASS: u32 = 3;
const FLAGS_UI: u32 = 10;

pub(super) fn target(port: u16) -> Settings {
    Settings::from([
        ("flags".to_owned(), Some("3".to_owned())),
        (
            "server".to_owned(),
            Some(format!("http=127.0.0.1:{port};https=127.0.0.1:{port}")),
        ),
        (
            "bypass".to_owned(),
            Some("localhost;127.0.0.1;[::1]".to_owned()),
        ),
    ])
}

fn query(flags: u32) -> Result<Settings, VpnFailure> {
    let mut values = [flags, SERVER, BYPASS].map(|option| OptionValue {
        option,
        value: Value { file_time: [0; 2] },
    });
    let mut list = Options {
        size: std::mem::size_of::<Options>() as u32,
        connection: null_mut(),
        count: values.len() as u32,
        error: 0,
        options: values.as_mut_ptr(),
    };
    let mut size = list.size;
    // WinINet owns only the returned string allocations, not the option array.
    let succeeded = unsafe {
        InternetQueryOptionW(
            null_mut(),
            PER_CONNECTION,
            (&mut list as *mut Options).cast(),
            &mut size,
        )
    };
    let error = (succeeded == 0).then(std::io::Error::last_os_error);
    let result = if let Some(error) = error {
        Err(failure(&format!("无法读取 Windows 系统代理：{error}")))
    } else {
        (|| {
            let mut settings = Settings::new();
            settings.insert(
                "flags".to_owned(),
                Some(unsafe { values[0].value.number }.to_string()),
            );
            for (key, value) in [("server", &values[1]), ("bypass", &values[2])] {
                let pointer = unsafe { value.value.string };
                let string = if pointer.is_null() {
                    String::new()
                } else {
                    let mut length = 0;
                    while length < 65536 && unsafe { *pointer.add(length) } != 0 {
                        length += 1;
                    }
                    if length == 65536 {
                        return Err(failure("Windows 系统代理字符串超限"));
                    }
                    String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, length) })
                        .map_err(|_| failure("Windows 系统代理字符串无效"))?
                };
                settings.insert(key.to_owned(), Some(string));
            }
            Ok(settings)
        })()
    };
    for value in &values[1..] {
        let pointer = unsafe { value.value.string };
        if !pointer.is_null() {
            unsafe {
                GlobalFree(pointer.cast());
            }
        }
    }
    result
}

pub(super) fn read() -> Result<Settings, VpnFailure> {
    query(FLAGS_UI).or_else(|_| query(FLAGS))
}

pub(super) fn write(settings: &Settings) -> Result<(), VpnFailure> {
    let mut strings: Vec<Vec<u16>> = vec![];
    let mut values = vec![];
    for (key, value) in settings {
        let value = value.as_deref().unwrap_or("");
        if value.contains('\0') || value.len() > 65536 {
            return Err(failure("Windows 系统代理值无效"));
        }
        let option = match key.as_str() {
            "flags" => FLAGS,
            "server" => SERVER,
            "bypass" => BYPASS,
            _ => return Err(failure("Windows 系统代理字段无效")),
        };
        let value = if option == FLAGS {
            Value {
                number: value.parse().map_err(|_| failure("Windows 代理模式无效"))?,
            }
        } else {
            strings.push(value.encode_utf16().chain(std::iter::once(0)).collect());
            Value {
                string: strings
                    .last_mut()
                    .ok_or_else(|| failure("代理字符串缺失"))?
                    .as_mut_ptr(),
            }
        };
        values.push(OptionValue { option, value });
    }
    if values.is_empty() {
        return Ok(());
    }
    let mut list = Options {
        size: std::mem::size_of::<Options>() as u32,
        connection: null_mut(),
        count: values.len() as u32,
        error: 0,
        options: values.as_mut_ptr(),
    };
    if unsafe {
        InternetSetOptionW(
            null_mut(),
            PER_CONNECTION,
            (&mut list as *mut Options).cast(),
            list.size,
        )
    } == 0
    {
        return Err(failure(&format!(
            "无法设置 Windows 系统代理：{}",
            std::io::Error::last_os_error()
        )));
    }
    for option in [39, 37] {
        if unsafe { InternetSetOptionW(null_mut(), option, null_mut(), 0) } == 0 {
            return Err(failure(&format!(
                "无法刷新 Windows 系统代理：{}",
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}
