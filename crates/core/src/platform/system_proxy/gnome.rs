use super::{failure, Settings};
use crate::connection::model::VpnFailure;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr::null_mut;

#[link(name = "gio-2.0")]
extern "C" {
    fn g_settings_schema_source_get_default() -> *mut c_void;
    fn g_settings_schema_source_lookup(
        source: *mut c_void,
        schema: *const c_char,
        recursive: i32,
    ) -> *mut c_void;
    fn g_settings_schema_unref(schema: *mut c_void);
    fn g_settings_new_full(
        schema: *mut c_void,
        backend: *mut c_void,
        path: *const c_char,
    ) -> *mut c_void;
    fn g_settings_get_user_value(settings: *mut c_void, key: *const c_char) -> *mut c_void;
    fn g_settings_is_writable(settings: *mut c_void, key: *const c_char) -> i32;
    fn g_settings_set_value(settings: *mut c_void, key: *const c_char, value: *mut c_void) -> i32;
    fn g_settings_reset(settings: *mut c_void, key: *const c_char);
    fn g_settings_sync();
}
#[link(name = "glib-2.0")]
extern "C" {
    fn g_variant_parse(
        kind: *const c_void,
        text: *const c_char,
        limit: *const c_char,
        end: *mut *const c_char,
        error: *mut *mut c_void,
    ) -> *mut c_void;
    fn g_variant_print(value: *mut c_void, annotate: i32) -> *mut c_char;
    fn g_variant_unref(value: *mut c_void);
    fn g_free(value: *mut c_void);
}
#[link(name = "gobject-2.0")]
extern "C" {
    fn g_object_unref(value: *mut c_void);
}

struct Handle(*mut c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            g_object_unref(self.0);
        }
    }
}

fn handle(schema: &str) -> Result<Handle, VpnFailure> {
    let schema = CString::new(schema).map_err(|_| failure("GNOME schema 无效"))?;
    let source = unsafe { g_settings_schema_source_get_default() };
    if source.is_null() {
        return Err(failure("系统没有安装 GNOME 代理配置 schema"));
    }
    let definition = unsafe { g_settings_schema_source_lookup(source, schema.as_ptr(), 1) };
    if definition.is_null() {
        return Err(failure("系统没有安装 GNOME 代理配置 schema"));
    }
    let settings = unsafe { g_settings_new_full(definition, null_mut(), null_mut()) };
    unsafe {
        g_settings_schema_unref(definition);
    }
    if settings.is_null() {
        return Err(failure("无法打开 GNOME 系统代理配置"));
    }
    Ok(Handle(settings))
}

fn field(key: &str) -> Result<(String, CString), VpnFailure> {
    let (schema, name) = key
        .rsplit_once('/')
        .ok_or_else(|| failure("GNOME 代理字段无效"))?;
    let allowed = match schema {
        "org.gnome.system.proxy" => ["mode", "ignore-hosts"].contains(&name),
        "org.gnome.system.proxy.http" => ["host", "port", "use-authentication"].contains(&name),
        "org.gnome.system.proxy.https" => ["host", "port"].contains(&name),
        _ => false,
    };
    if !allowed {
        return Err(failure("GNOME 代理字段不支持"));
    }
    Ok((
        schema.to_owned(),
        CString::new(name).map_err(|_| failure("GNOME 代理字段无效"))?,
    ))
}

pub(super) fn available() -> Result<(), VpnFailure> {
    if std::env::var("GSETTINGS_BACKEND")
        .ok()
        .is_some_and(|backend| !backend.is_empty() && backend != "dconf")
    {
        return Err(failure(
            "系统代理需要桌面 dconf 后端，不支持进程私有设置后端",
        ));
    }
    if std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .unwrap_or_default()
        .is_empty()
    {
        return Err(failure("当前会话没有可用的桌面 D-Bus"));
    }
    for schema in [
        "org.gnome.system.proxy",
        "org.gnome.system.proxy.http",
        "org.gnome.system.proxy.https",
    ] {
        handle(schema)?;
    }
    Ok(())
}

pub(super) fn target(port: u16) -> Settings {
    Settings::from([
        (
            "org.gnome.system.proxy/mode".to_owned(),
            Some("'manual'".to_owned()),
        ),
        (
            "org.gnome.system.proxy/ignore-hosts".to_owned(),
            Some("['localhost', '127.0.0.0/8', '::1']".to_owned()),
        ),
        (
            "org.gnome.system.proxy.http/host".to_owned(),
            Some("'127.0.0.1'".to_owned()),
        ),
        (
            "org.gnome.system.proxy.http/port".to_owned(),
            Some(port.to_string()),
        ),
        (
            "org.gnome.system.proxy.http/use-authentication".to_owned(),
            Some("false".to_owned()),
        ),
        (
            "org.gnome.system.proxy.https/host".to_owned(),
            Some("'127.0.0.1'".to_owned()),
        ),
        (
            "org.gnome.system.proxy.https/port".to_owned(),
            Some(port.to_string()),
        ),
    ])
}

pub(super) fn read() -> Result<Settings, VpnFailure> {
    let mut result = Settings::new();
    for key in target(1).keys() {
        let (schema, name) = field(key)?;
        let settings = handle(&schema)?;
        let value = unsafe { g_settings_get_user_value(settings.0, name.as_ptr()) };
        let text = if value.is_null() {
            None
        } else {
            let text = unsafe { g_variant_print(value, 0) };
            if text.is_null() {
                unsafe {
                    g_variant_unref(value);
                }
                return Err(failure("无法编码 GNOME 代理配置"));
            }
            let output = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                g_free(text.cast());
                g_variant_unref(value);
            }
            Some(output)
        };
        result.insert(key.clone(), text);
    }
    Ok(result)
}

pub(super) fn write(values: &Settings) -> Result<(), VpnFailure> {
    // Enable mode last; restore the previous mode before changing endpoints.
    let mode = "org.gnome.system.proxy/mode";
    let enabling = values.get(mode).and_then(Option::as_deref) == Some("'manual'");
    let mut keys: Vec<_> = values.keys().collect();
    keys.sort_by_key(|key| {
        if key.as_str() == mode {
            if enabling {
                2
            } else {
                0
            }
        } else {
            1
        }
    });
    for key in keys {
        let (schema, name) = field(key)?;
        let settings = handle(&schema)?;
        if unsafe { g_settings_is_writable(settings.0, name.as_ptr()) } == 0 {
            return Err(failure("GNOME 系统代理被策略锁定"));
        }
        match &values[key] {
            None => unsafe {
                g_settings_reset(settings.0, name.as_ptr());
            },
            Some(value) => {
                let text = CString::new(value.as_str()).map_err(|_| failure("GNOME 代理值无效"))?;
                // GLib's G_VARIANT_TYPE_* macros use these static type strings.
                // An empty string array cannot be inferred from "[]" alone.
                let kind: &[u8] = match name.as_bytes() {
                    b"ignore-hosts" => b"as\0",
                    b"port" => b"i\0",
                    b"use-authentication" => b"b\0",
                    _ => b"s\0",
                };
                let variant = unsafe {
                    g_variant_parse(
                        kind.as_ptr().cast(),
                        text.as_ptr(),
                        null_mut(),
                        null_mut(),
                        null_mut(),
                    )
                };
                if variant.is_null() {
                    return Err(failure("GNOME 代理值无法解析"));
                }
                let accepted = unsafe { g_settings_set_value(settings.0, name.as_ptr(), variant) };
                unsafe {
                    g_variant_unref(variant);
                }
                if accepted == 0 {
                    return Err(failure("GNOME 系统代理写入失败"));
                }
            }
        }
        unsafe {
            g_settings_sync();
        }
    }
    Ok(())
}
