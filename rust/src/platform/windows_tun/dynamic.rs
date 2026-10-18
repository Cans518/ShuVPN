use super::ip::{same_owned_route, same_prefix, RouteIo};
use crate::platform::model::PlatformFailure;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    time::{Duration, Instant},
};
use windows_sys::Win32::NetworkManagement::{IpHelper::MIB_IPFORWARD_ROW2, Ndis::NET_LUID_LH};

#[derive(Default)]
pub(super) struct Routes {
    owned: BTreeMap<IpAddr, MIB_IPFORWARD_ROW2>,
    failed: bool,
}

impl Routes {
    pub(super) fn sync(
        &mut self,
        io: &mut impl RouteIo,
        wanted: BTreeSet<IpAddr>,
        luid: NET_LUID_LH,
        index: u32,
        mut check: impl FnMut() -> Result<(), PlatformFailure>,
    ) -> Result<(), PlatformFailure> {
        if self.failed {
            return Err(fatal("上次动态路由事务未恢复，必须撤销 TUN"));
        }
        check()?;
        if let Err(error) = verify(io, &self.owned) {
            self.failed = true;
            return Err(fatal(&format!("当前捕获路由已无法确认：{}", error.message)));
        }
        let before = self.owned.clone();
        let original: BTreeSet<_> = before.keys().copied().collect();
        let applied = (|| {
            for ip in wanted.difference(&original) {
                check()?;
                let network = ipnet::IpNet::new(*ip, if ip.is_ipv4() { 32 } else { 128 })
                    .map_err(|_| PlatformFailure::new("route", "动态捕获地址无效"))?;
                let row = super::ip::route_row(luid, index, network);
                ensure_available(io, &row)?;
                check()?;
                io.add(&row)?;
                self.owned.insert(*ip, row);
            }
            for ip in original.difference(&wanted) {
                check()?;
                let row = before.get(ip).ok_or_else(|| fatal("动态路由记录丢失"))?;
                io.remove(row)?;
                self.owned.remove(ip);
            }
            check()?;
            verify(io, &self.owned)?;
            check()
        })();
        if let Err(error) = applied {
            if let Err(rollback) = self.restore(io, &before) {
                self.failed = true;
                return Err(fatal(&format!(
                    "{}；恢复失败：{}",
                    error.message, rollback.message
                )));
            }
            return Err(error);
        }
        Ok(())
    }

    fn restore(
        &mut self,
        io: &mut impl RouteIo,
        before: &BTreeMap<IpAddr, MIB_IPFORWARD_ROW2>,
    ) -> Result<(), PlatformFailure> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let changed = self.owned.clone();
        let mut failure = None;
        for (ip, row) in changed
            .iter()
            .rev()
            .filter(|(ip, _)| !before.contains_key(ip))
        {
            if Instant::now() >= deadline {
                return Err(fatal("回滚动态路由超时"));
            }
            match io.remove(row) {
                Ok(()) => {
                    self.owned.remove(ip);
                }
                Err(error) => failure = Some(error),
            }
        }
        for (ip, row) in before.iter().filter(|(ip, _)| !changed.contains_key(ip)) {
            if Instant::now() >= deadline {
                return Err(fatal("恢复动态路由超时"));
            }
            match ensure_available(io, row).and_then(|_| io.add(row)) {
                Ok(()) => {
                    self.owned.insert(*ip, *row);
                }
                Err(error) => failure = Some(error),
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        verify(io, before)?;
        if Instant::now() >= deadline {
            return Err(fatal("动态路由恢复确认超时"));
        }
        Ok(())
    }

    pub(super) fn close(&mut self, io: &mut impl RouteIo) -> Result<(), PlatformFailure> {
        let mut failure = None;
        for (ip, row) in self.owned.clone().iter().rev() {
            match io.remove(row) {
                Ok(()) => {
                    self.owned.remove(ip);
                }
                Err(error) => failure = Some(error),
            }
        }
        if let Some(error) = failure {
            return Err(PlatformFailure::new(
                "cleanup",
                &format!("动态捕获路由清理失败：{}", error.message),
            ));
        }
        self.failed = false;
        Ok(())
    }
}

fn ensure_available(
    io: &mut impl RouteIo,
    row: &MIB_IPFORWARD_ROW2,
) -> Result<(), PlatformFailure> {
    if io
        .entries()?
        .iter()
        .any(|existing| same_prefix(existing, row))
    {
        return Err(PlatformFailure::new(
            "conflict",
            "动态捕获不能覆盖现有主机路由",
        ));
    }
    Ok(())
}

fn verify(
    io: &mut impl RouteIo,
    owned: &BTreeMap<IpAddr, MIB_IPFORWARD_ROW2>,
) -> Result<(), PlatformFailure> {
    if owned.is_empty() {
        return Ok(());
    }
    let entries = io.entries()?;
    for row in owned.values() {
        let matching: Vec<_> = entries
            .iter()
            .filter(|existing| same_prefix(existing, row))
            .collect();
        if matching.len() != 1 || !same_owned_route(matching[0], row) {
            return Err(PlatformFailure::new(
                "conflict",
                "动态捕获路由丢失、被修改或被外部路由遮蔽",
            ));
        }
    }
    Ok(())
}
fn fatal(message: &str) -> PlatformFailure {
    PlatformFailure::new("route_rollback", message)
}

#[cfg(test)]
mod tests {
    use super::super::ip::{address, route_row};
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[derive(Default)]
    struct Kernel {
        rows: Vec<MIB_IPFORWARD_ROW2>,
        add_calls: usize,
        fail_add: BTreeSet<usize>,
        fail_remove: BTreeSet<IpAddr>,
        cancel_after_add: Option<CancellationToken>,
        writes: Vec<(String, IpAddr)>,
    }
    impl RouteIo for Kernel {
        fn entries(&mut self) -> Result<Vec<MIB_IPFORWARD_ROW2>, PlatformFailure> {
            Ok(self.rows.clone())
        }
        fn add(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure> {
            self.add_calls += 1;
            if self.fail_add.contains(&self.add_calls) {
                return Err(PlatformFailure::new("system", "injected create failure"));
            }
            if self.rows.iter().any(|existing| same_prefix(existing, row)) {
                return Err(PlatformFailure::new("conflict", "occupied prefix"));
            }
            self.rows.push(*row);
            self.writes.push(("add".to_owned(), host(row)?));
            if let Some(token) = self.cancel_after_add.take() {
                token.cancel();
            }
            Ok(())
        }
        fn remove(&mut self, row: &MIB_IPFORWARD_ROW2) -> Result<(), PlatformFailure> {
            let ip = host(row)?;
            if self.fail_remove.contains(&ip) {
                return Err(PlatformFailure::new("system", "injected delete failure"));
            }
            let at = self.rows.iter().position(|existing| {
                same_prefix(existing, row)
                    && unsafe { existing.InterfaceLuid.Value == row.InterfaceLuid.Value }
            });
            if let Some(at) = at {
                if !same_owned_route(&self.rows[at], row) {
                    return Err(PlatformFailure::new("conflict", "external modification"));
                }
                self.rows.remove(at);
                self.writes.push(("remove".to_owned(), ip));
            }
            Ok(())
        }
    }
    fn host(row: &MIB_IPFORWARD_ROW2) -> Result<IpAddr, PlatformFailure> {
        address(&row.DestinationPrefix.Prefix).ok_or_else(|| fatal("bad test row"))
    }
    fn set(values: &[&str]) -> anyhow::Result<BTreeSet<IpAddr>> {
        values
            .iter()
            .map(|value| value.parse().map_err(anyhow::Error::from))
            .collect()
    }
    fn sync(routes: &mut Routes, kernel: &mut Kernel, values: &[&str]) -> anyhow::Result<()> {
        routes.sync(
            kernel,
            set(values)?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        )?;
        Ok(())
    }
    fn assert_rows(kernel: &Kernel, values: &[&str]) -> anyhow::Result<()> {
        let actual: BTreeSet<_> = kernel.rows.iter().map(host).collect::<Result<_, _>>()?;
        assert_eq!(actual, set(values)?);
        Ok(())
    }

    #[test]
    fn dual_stack_host_routes_are_exclusive_and_close_removes_only_owned_rows() -> anyhow::Result<()>
    {
        let mut kernel = Kernel::default();
        let foreign = route_row(NET_LUID_LH { Value: 77 }, 9, "192.0.2.9/32".parse()?);
        kernel.rows.push(foreign);
        let mut routes = Routes::default();
        sync(&mut routes, &mut kernel, &["10.99.0.7", "2001:db8::7"])?;
        let v6 = routes
            .owned
            .get(&"2001:db8::7".parse()?)
            .ok_or_else(|| anyhow::anyhow!("missing v6"))?;
        assert_eq!(v6.DestinationPrefix.PrefixLength, 128);
        assert_eq!(unsafe { v6.InterfaceLuid.Value }, 12345);
        assert_eq!(v6.InterfaceIndex, 321);
        let writes = kernel.writes.len();
        sync(&mut routes, &mut kernel, &["10.99.0.7", "2001:db8::7"])?;
        assert_eq!(kernel.writes.len(), writes);
        sync(&mut routes, &mut kernel, &["2001:db8::7"])?;
        routes.close(&mut kernel)?;
        routes.close(&mut kernel)?;
        assert_rows(&kernel, &["192.0.2.9"])?;
        assert!(same_owned_route(&kernel.rows[0], &foreign));
        Ok(())
    }

    #[test]
    fn partial_create_and_delete_failures_restore_the_last_valid_set() -> anyhow::Result<()> {
        let mut kernel = Kernel::default();
        let mut routes = Routes::default();
        sync(&mut routes, &mut kernel, &["10.99.0.7"])?;
        kernel.fail_add.insert(3);
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.8", "10.99.0.9"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("system".to_owned())
        );
        assert_rows(&kernel, &["10.99.0.7"])?;
        kernel.fail_add.clear();
        kernel.fail_remove.insert("10.99.0.7".parse()?);
        assert!(routes
            .sync(
                &mut kernel,
                set(&["10.99.0.8"])?,
                NET_LUID_LH { Value: 12345 },
                321,
                || Ok(())
            )
            .is_err());
        assert_rows(&kernel, &["10.99.0.7"])?;
        kernel.fail_remove.clear();
        sync(&mut routes, &mut kernel, &["10.99.0.8"])?;
        assert_rows(&kernel, &["10.99.0.8"])?;
        Ok(())
    }

    #[test]
    fn cancellation_after_a_kernel_write_restores_old_rows_before_returning() -> anyhow::Result<()>
    {
        let mut routes = Routes::default();
        let mut kernel = Kernel::default();
        sync(&mut routes, &mut kernel, &["10.99.0.7"])?;
        let cancel = CancellationToken::new();
        kernel.cancel_after_add = Some(cancel.clone());
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.7", "10.99.0.8"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || {
                if cancel.is_cancelled() {
                    Err(PlatformFailure::new("cancelled", "test cancellation"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("cancelled".to_owned())
        );
        assert_rows(&kernel, &["10.99.0.7"])?;
        assert!(!routes.failed);
        Ok(())
    }

    #[test]
    fn a_late_delete_failure_recreates_removed_old_routes_and_reports_failed_recreation(
    ) -> anyhow::Result<()> {
        let mut routes = Routes::default();
        let mut kernel = Kernel::default();
        sync(&mut routes, &mut kernel, &["10.99.0.7", "10.99.0.8"])?;
        kernel.fail_remove.insert("10.99.0.8".parse()?);
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.9"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("system".to_owned())
        );
        assert_rows(&kernel, &["10.99.0.7", "10.99.0.8"])?;
        kernel.fail_add.insert(kernel.add_calls + 2);
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.9"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("route_rollback".to_owned())
        );
        assert_rows(&kernel, &["10.99.0.8"])?;
        assert!(routes.failed);
        kernel.fail_remove.clear();
        routes.close(&mut kernel)?;
        assert_rows(&kernel, &[])?;
        Ok(())
    }

    #[test]
    fn rollback_failure_is_fatal_and_failed_cleanup_keeps_retryable_ownership() -> anyhow::Result<()>
    {
        let mut routes = Routes::default();
        let mut kernel = Kernel::default();
        sync(&mut routes, &mut kernel, &["10.99.0.7"])?;
        kernel.fail_add.insert(3);
        kernel.fail_remove.insert("10.99.0.8".parse()?);
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.8", "10.99.0.9"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("route_rollback".to_owned())
        );
        assert!(routes.failed);
        assert!(routes.close(&mut kernel).is_err());
        assert_eq!(
            routes.owned.keys().copied().collect::<BTreeSet<_>>(),
            set(&["10.99.0.8"])?
        );
        let calls = kernel.add_calls;
        assert!(routes
            .sync(
                &mut kernel,
                set(&[])?,
                NET_LUID_LH { Value: 12345 },
                321,
                || Ok(())
            )
            .is_err());
        assert_eq!(kernel.add_calls, calls);
        kernel.fail_remove.clear();
        routes.close(&mut kernel)?;
        assert!(routes.owned.is_empty());
        assert!(!routes.failed);
        assert_rows(&kernel, &[])?;
        Ok(())
    }

    #[test]
    fn foreign_routes_and_external_modifications_are_preserved() -> anyhow::Result<()> {
        let mut routes = Routes::default();
        let mut kernel = Kernel::default();
        let foreign = route_row(NET_LUID_LH { Value: 77 }, 99, "10.99.0.8/32".parse()?);
        kernel.rows.push(foreign);
        sync(&mut routes, &mut kernel, &["10.99.0.7"])?;
        let failed = routes.sync(
            &mut kernel,
            set(&["10.99.0.7", "10.99.0.8"])?,
            NET_LUID_LH { Value: 12345 },
            321,
            || Ok(()),
        );
        assert_eq!(
            failed.err().map(|error| error.code),
            Some("conflict".to_owned())
        );
        assert!(kernel
            .rows
            .iter()
            .any(|row| same_owned_route(row, &foreign)));
        let owned_ip: IpAddr = "10.99.0.7".parse()?;
        let owned_at = kernel
            .rows
            .iter()
            .position(|row| host(row).ok() == Some(owned_ip))
            .ok_or_else(|| anyhow::anyhow!("missing owned row"))?;
        kernel.rows[owned_at].Metric = 100;
        assert!(routes
            .sync(
                &mut kernel,
                set(&["10.99.0.7"])?,
                NET_LUID_LH { Value: 12345 },
                321,
                || Ok(())
            )
            .is_err());
        assert!(routes.failed);
        assert!(routes.close(&mut kernel).is_err());
        assert_eq!(kernel.rows[owned_at].Metric, 100);
        assert_eq!(routes.owned.len(), 1);
        // A disappeared owned row is safe to forget without touching the foreign interface.
        kernel.rows.remove(owned_at);
        routes.close(&mut kernel)?;
        assert_rows(&kernel, &["10.99.0.8"])?;
        Ok(())
    }
}
