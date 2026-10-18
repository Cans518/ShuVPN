//! Independent observer: no adapter, route, process termination or elevation calls.
use super::{
    full_observation::{pipe_name, pipe_process, read_frame, write_frame, RPC_BUDGET},
    full_process::{sha_file, Identity, Process},
};
use anyhow::{ensure, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};
use tokio::net::windows::named_pipe::ClientOptions;
use windows_sys::Win32::{
    NetworkManagement::IpHelper::GetCurrentThreadCompartmentId,
    Storage::FileSystem::{SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT},
    System::Threading::CREATE_NO_WINDOW,
};

fn stages(mode: &str) -> anyhow::Result<Vec<(&'static str, usize)>> {
    match mode {
        "--plan" => Ok(vec![("begin", 0), ("cleaned", 0), ("finish", 0)]),
        "--run-privileged" | "--run-local-elevated" => Ok(vec![
            ("begin", 0),
            ("ready", 0),
            ("cleaned", 0),
            ("ready", 1),
            ("cleaned", 1),
            ("finish", 0),
        ]),
        _ => anyhow::bail!("invalid observer mode"),
    }
}

fn validate_request(
    request: &Value,
    token: &str,
    pid: u32,
    sequence: usize,
    expected: (&str, usize),
) -> anyhow::Result<()> {
    ensure!(
        request["token"] == token && request["pid"] == pid && request["sequence"] == sequence,
        "observer request identity/sequence mismatch"
    );
    ensure!(
        request["stage"] == expected.0 && request["round"] == expected.1,
        "observer stage/round out of order"
    );
    Ok(())
}

async fn snapshot(scope: &str, sid: &str) -> anyhow::Result<Value> {
    ensure!(
        ["Current", "All"].contains(&scope)
            && sid.starts_with("S-1-")
            && sid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-')),
        "invalid snapshot scope/SID"
    );
    // Embed the reviewed query so the observed executable's SHA also pins it.
    // The privileged observer never executes a mutable client-supplied script.
    let script = format!(
        "& {{\n{}\n}} -Scope '{scope}' -UserSid '{sid}' -Utf8Stdout",
        include_str!("../../../tool/full_capture_windows_snapshot.ps1")
    );
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let shell = PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot absent")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe")
        .canonicalize()?;
    let output = tokio::time::timeout(
        Duration::from_secs(25),
        tokio::process::Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
            ])
            .arg(encoded)
            .creation_flags(CREATE_NO_WINDOW)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("network observer snapshot timed out")??;
    ensure!(
        output.status.success(),
        "network observer query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = output
        .stdout
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&output.stdout);
    // The snapshot contains ASCII property names but may contain localized interface names.
    let document: Value =
        serde_json::from_slice(bytes).context("network snapshot is not UTF-8 JSON")?;
    for key in ["routes", "addresses", "interfaces", "adapters", "dns"] {
        ensure!(
            document[key]
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "independent snapshot has an empty {key} table"
        );
    }
    Ok(document)
}

fn installed_routes(
    snapshot: &Value,
    status: &Value,
    plan: &Value,
    compartment: u64,
) -> anyhow::Result<usize> {
    let index = status["interface"]["index"]
        .as_u64()
        .context("active interface index absent")?;
    ensure!(index > 0, "zero active interface index");
    let mut owned = BTreeMap::new();
    for route in snapshot["routes"].as_array().context("routes absent")? {
        if route["InterfaceIndex"] == index && route["CompartmentId"] == compartment {
            let prefix: ipnet::IpNet = route["DestinationPrefix"]
                .as_str()
                .context("route prefix absent")?
                .parse()?;
            *owned.entry(prefix.trunc()).or_insert(0usize) += 1;
        }
    }
    let wanted = plan["routes"]
        .as_array()
        .context("approved routes absent")?;
    ensure!(!wanted.is_empty(), "approved Full routes empty");
    for route in wanted {
        let network: ipnet::IpNet = route
            .as_str()
            .context("approved route is not text")?
            .parse()?;
        ensure!(
            owned.get(&network.trunc()) == Some(&1),
            "Full route missing/duplicated on exact interface"
        );
    }
    Ok(wanted.len())
}

struct Witness {
    process: Process,
    identity: Identity,
    round: usize,
    exited: bool,
}

pub async fn run(mode: &str, token: &str) -> anyhow::Result<()> {
    let expected = stages(mode)?;
    let scope = if mode == "--plan" { "Current" } else { "All" };
    let observer = Process::open(std::process::id())?.identity()?;
    ensure!(
        mode == "--plan" || observer.elevated,
        "runtime observer must already be elevated"
    );
    let image = std::env::current_exe()?.canonicalize()?;
    let helper = image
        .parent()
        .context("observer image directory")?
        .join("shuvpn_tun_helper.exe");
    let helper_sha = if mode == "--run-privileged" {
        Some(sha_file(&helper)?)
    } else {
        None
    };
    let name = pipe_name(token)?;
    println!(
        "WINDOWS_FULL_OBSERVER_READY {}",
        json!({"token":token,"mode":mode,"observer":observer})
    );
    let mut pipe = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match ClientOptions::new()
                .security_qos_flags(SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT)
                .open(&name)
            {
                Ok(pipe) => return Ok::<_, anyhow::Error>(pipe),
                Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) => {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await
    .context("observer pipe connection timeout")??;
    let probe = pipe_process(&pipe, true)?;
    let identity = probe.identity()?;
    ensure!(
        identity.pid != observer.pid
            && identity.image == observer.image
            && identity.sha256 == observer.sha256,
        "pipe client is not a separate matching probe executable"
    );
    ensure!(
        mode != "--run-privileged" || !identity.elevated,
        "helper probe must be an ordinary process"
    );
    ensure!(
        mode != "--run-local-elevated" || identity.elevated,
        "local probe must be elevated"
    );
    let mut report = json!({"token":token,"mode":mode,"scope":scope,
        "observer_identity":observer,"probe_identity":identity,"passed":false,"helpers":[]});
    let mut baseline = Value::Null;
    let mut witnesses: Vec<Witness> = vec![];
    let mut seen = BTreeSet::new();
    let mut active = None;
    let result = async {
        for (sequence, expected_stage) in expected.iter().copied().enumerate() {
            let request = tokio::time::timeout(RPC_BUDGET, read_frame(&mut pipe))
                .await.context("observer request timeout")??;
            let checked = async {
                validate_request(&request, token, identity.pid, sequence, expected_stage)?;
                ensure!(probe.alive()?, "probe exited before its observation barrier");
                let data = match expected_stage.0 {
                    "begin" => {
                        ensure!(request["data"]["mode"] == mode, "client requested another observation mode");
                        if scope == "Current" {
                            // SAFETY: read-only current-thread network compartment query.
                            let compartment = unsafe { GetCurrentThreadCompartmentId() };
                            ensure!(request["compartment_id"] == compartment,
                                "preview observer and probe are in different compartments");
                        }
                        baseline = snapshot(scope, &identity.sid).await?;
                        report["before"] = baseline.clone();
                        json!({"scope":scope,"baseline":baseline})
                    }
                    "ready" => {
                        let status = &request["data"]["status"];
                        let plan = &request["data"]["plan"];
                        ensure!(status["state"] == "running" && status["capture_mode"] == "full"
                            && plan["capture_mode"] == "full", "active Full evidence absent");
                        let helper_identity = if mode == "--run-privileged" {
                            ensure!(active.is_none() && status["helperIo"]["mode"] == "helper",
                                "helper witness state mismatch");
                            let process = &status["helperIo"]["process"];
                            let pid = u32::try_from(process["pid"].as_u64().context("helper PID absent")?)?;
                            let retained = Process::open(pid)?;
                            let actual = retained.identity()?;
                            ensure!(actual.created_ticks == process["created_ticks"]
                                && actual.image == helper.canonicalize()?.to_string_lossy().as_ref()
                                && Some(&actual.sha256) == helper_sha.as_ref() && actual.elevated,
                                "helper retained identity/image/elevation mismatch");
                            ensure!(actual.pid != identity.pid && actual.pid != observer.pid
                                && seen.insert((actual.pid,actual.created_ticks.clone())),
                                "helper instance reused or not separate");
                            witnesses.push(Witness { process:retained,identity:actual.clone(),
                                round:expected_stage.1,exited:false });
                            active = Some(witnesses.len()-1);
                            serde_json::to_value(actual)?
                        } else {
                            ensure!(status["helperIo"]["mode"] == "local", "local mode reported a helper");
                            Value::Null
                        };
                        let current = snapshot(scope, &identity.sid).await?;
                        let count = installed_routes(&current,status,plan,
                            request["compartment_id"].as_u64().context("compartment absent")?)?;
                        json!({"helper":helper_identity,"installed_routes":count,"snapshot":current})
                    }
                    "cleaned" => {
                        if mode == "--run-privileged" {
                            let witness = &mut witnesses[active.take().context("missing live helper witness")?];
                            ensure!(witness.round == expected_stage.1, "helper cleanup round mismatch");
                            witness.process.successful_exit().await?;
                            witness.exited = true;
                        }
                        let after = snapshot(scope,&identity.sid).await?;
                        ensure!(after == baseline, "independent network configuration did not return to baseline");
                        report["after"] = after.clone();
                        json!({"network_restored":true,"snapshot":after})
                    }
                    "finish" => {
                        ensure!(active.is_none() && (mode != "--run-privileged"
                            || (witnesses.len() == 2 && witnesses.iter().all(|witness| witness.exited))),
                            "helper lifetime evidence incomplete");
                        json!({"network_restored":true,"completed_stages":expected.len()})
                    }
                    _ => anyhow::bail!("unknown observation stage"),
                };
                Ok::<_,anyhow::Error>(data)
            }.await;
            let mut reply = json!({"token":token,"sequence":sequence,"stage":request["stage"],
                "observer_identity":observer,"ok":checked.is_ok()});
            match &checked {
                Ok(data) => reply["data"] = data.clone(),
                Err(ref error) => reply["error"] = json!(format!("{error:#}")),
            }
            tokio::time::timeout(RPC_BUDGET, write_frame(&mut pipe,&reply)).await
                .context("observer response timeout")??;
            checked?;
        }
        // The final acknowledgement lets the probe exit; retain the same handle until then.
        probe.successful_exit().await?;
        let final_snapshot = snapshot(scope, &identity.sid).await?;
        ensure!(final_snapshot == baseline, "network changed during final probe exit");
        report["after"] = final_snapshot;
        report["probe_exit_code"] = json!(0);
        report["network_restored"] = json!(report["before"] == report["after"]);
        report["passed"] = json!(true);
        Ok::<_,anyhow::Error>(())
    }.await;
    if let Err(error) = &result {
        report["error"] = json!(format!("{error:#}"));
        // Failure is never promoted to a pass, even if later cleanup succeeds.
        for witness in &mut witnesses {
            if !witness.exited {
                witness.exited = witness.process.successful_exit().await.is_ok();
            }
        }
        match snapshot(scope, &identity.sid).await {
            Ok(after) => report["after_failure"] = after,
            Err(error) => report["snapshot_error"] = json!(format!("{error:#}")),
        }
    }
    report["helpers"] = json!(witnesses
        .iter()
        .map(|witness| json!({
            "round":witness.round,"identity":witness.identity,"successful_exit":witness.exited,
        }))
        .collect::<Vec<_>>());
    println!("WINDOWS_FULL_OBSERVER_COMPLETE {report}");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_skipped_or_foreign_observation_messages_cannot_advance() -> anyhow::Result<()> {
        let request = json!({"token":"token","pid":42,"sequence":2,"stage":"cleaned","round":0});
        validate_request(&request, "token", 42, 2, ("cleaned", 0))?;
        for (token, pid, sequence, stage) in [
            ("other", 42, 2, ("cleaned", 0)),
            ("token", 43, 2, ("cleaned", 0)),
            ("token", 42, 3, ("cleaned", 0)),
            ("token", 42, 2, ("ready", 0)),
            ("token", 42, 2, ("cleaned", 1)),
        ] {
            assert!(validate_request(&request, token, pid, sequence, stage).is_err());
        }
        assert_eq!(stages("--run-privileged")?.len(), 6);
        assert_eq!(stages("--plan")?.len(), 3);
        Ok(())
    }

    #[test]
    fn route_witness_requires_one_owned_route_in_the_exact_compartment() -> anyhow::Result<()> {
        let status = json!({"interface":{"index":77}});
        let plan = json!({"routes":["198.51.100.0/24","2001:db8:ffff::/64"]});
        let routes = json!({"routes":[
            {"InterfaceIndex":77,"CompartmentId":1,"DestinationPrefix":"198.51.100.0/24"},
            {"InterfaceIndex":77,"CompartmentId":1,"DestinationPrefix":"2001:db8:ffff::/64"}
        ]});
        assert_eq!(installed_routes(&routes, &status, &plan, 1)?, 2);
        assert!(installed_routes(&routes, &status, &plan, 2).is_err());
        let mut duplicate = routes.clone();
        duplicate["routes"]
            .as_array_mut()
            .context("routes")?
            .push(routes["routes"][0].clone());
        assert!(installed_routes(&duplicate, &status, &plan, 1).is_err());
        let mut missing = routes;
        missing["routes"].as_array_mut().context("routes")?.pop();
        assert!(installed_routes(&missing, &status, &plan, 1).is_err());
        Ok(())
    }
}
