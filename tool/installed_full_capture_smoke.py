"""Run the installed default core through an ordinary-user Flutter engine."""
import argparse
import grp
import hashlib
import json
import os
from pathlib import Path
import pwd
import select
import signal
import stat
import subprocess
import sys
import time
import uuid

from tun_data_plane_linux_smoke import PEER, line

LIBRARY = Path("/opt/shuvpn/lib/libshuvpn_core.so")
HELPER = Path("/usr/lib/shuvpn/shuvpn_tun_helper")
STAGES = (
    "automatic-ready", "before-blocked", "after-blocked", "coverage-change",
    "automatic-cleanup", "manual-crash-ready", "helper-crash",
    "manual-crash-cleanup", "manual-normal-ready", "manual-normal-cleanup",
)

def fixture_environment(base):
    environment = {key: value for key, value in base.items()
                   if key.lower() not in {"http_proxy", "https_proxy", "all_proxy", "no_proxy"}}
    environment["NO_PROXY"] = "*"
    environment["no_proxy"] = "*"
    return environment

def validate_stage(completed, stage):
    if len(completed) >= len(STAGES) or stage != STAGES[len(completed)]:
        raise ValueError("unexpected or repeated installed Full stage")
    return STAGES[len(completed)]


def validate_complete(completed):
    if list(completed) != list(STAGES):
        raise ValueError("installed Full faults or observations were omitted")


def acknowledge(directory, stage, token):
    if stage not in STAGES:
        raise ValueError("invalid barrier filename")
    descriptor = os.open(stage, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o644, dir_fd=directory)
    with os.fdopen(descriptor, "w") as output:
        output.write(token)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def ip(*args):
    return subprocess.check_output(["/usr/sbin/ip", *args], text=True)


def namespace(kind, pid="self"):
    return os.readlink(f"/proc/{pid}/ns/{kind}")


def routing_state():
    return {
        "routes": [json.loads(ip("-j", family, "route", "show", "table", "all"))
                   for family in ("-4", "-6")],
        "rules": [json.loads(ip("-j", family, "rule", "show"))
                  for family in ("-4", "-6")],
    }

def check_routing(baseline):
    current = routing_state()
    if current != baseline:
        print("INSTALLED_FULL_ROUTING_MISMATCH " +
              json.dumps({"before": baseline, "after": current}), flush=True)
        raise AssertionError("TUN cleanup did not restore routes and rules")

def wait_addresses():
    """Take the restoration baseline after the fixture's link-local DAD."""
    deadline = time.monotonic() + 5
    while True:
        rows = json.loads(ip("-j", "-6", "address", "show", "dev", "shuphys"))
        addresses = [address for row in rows for address in row.get("addr_info", [])]
        if any(address.get("dadfailed", False) for address in addresses):
            raise RuntimeError("physical fixture IPv6 duplicate-address detection failed")
        if not any(address.get("tentative", False) for address in addresses):
            return
        if time.monotonic() >= deadline:
            raise RuntimeError("physical fixture IPv6 addresses did not settle")
        time.sleep(0.02)


def helper_pids():
    result = set()
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                if os.readlink(entry / "exe") == str(HELPER):
                    result.add(int(entry.name))
            except (FileNotFoundError, ProcessLookupError):
                pass
    return result


def exited(descriptor):
    return bool(select.select([descriptor], [], [], 0)[0])


def identity(pid, parent, expected, network):
    descriptor = os.pidfd_open(pid)
    try:
        assert not exited(descriptor), "helper exited before identity observation"
        status = dict(line.split(":", 1) for line in
                      Path(f"/proc/{pid}/status").read_text().splitlines())
        assert int(status["PPid"]) == parent
        assert list(map(int, status["Uid"].split())) == [expected] * 4
        assert int(status["CapEff"].strip(), 16) & 0x3000 == 0x3000
        assert namespace("net", pid) == network
        executable = Path(f"/proc/{pid}/exe")
        assert os.readlink(executable) == str(HELPER)
        assert (executable.stat().st_dev, executable.stat().st_ino) == (
            HELPER.stat().st_dev, HELPER.stat().st_ino)
        assert sha256(executable) == sha256(HELPER)
        assert b"--serve-fd\x003\x00" in Path(f"/proc/{pid}/cmdline").read_bytes()
        assert not exited(descriptor), "helper changed during identity observation"
        return descriptor, {
            "pid": pid, "parent": parent, "exe": str(HELPER),
            "sha256": sha256(executable), "net": network,
            "uid": expected, "cap_eff": status["CapEff"].strip(),
        }
    except BaseException:
        os.close(descriptor)
        raise


def run(args):
    if not __debug__:
        raise RuntimeError("Run the installed Full verifier with assertions enabled")
    assert os.geteuid() == 0, "root prepares the isolated test; the engine is unprivileged"
    for kind in ("net", "mnt"):
        assert namespace(kind) != os.environ[f"SHUVPN_INSTALL_PARENT_{kind.upper()}"]
    account = pwd.getpwnam(args.account)
    assert account.pw_uid != 0
    report = args.report.resolve()
    assert not report.exists(), "use a fresh report directory"
    report.mkdir(mode=0o755)
    work = report / "engine"
    work.mkdir(mode=0o700)
    os.chown(work, account.pw_uid, account.pw_gid)
    barriers = report / "barriers"
    barriers.mkdir(mode=0o755)
    barrier_directory = os.open(barriers, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    helper_stat = HELPER.stat()
    assert (helper_stat.st_uid, helper_stat.st_gid, stat.S_IMODE(helper_stat.st_mode)) == (
        0, grp.getgrnam("shuvpn").gr_gid, 0o750)
    capabilities = subprocess.check_output(["getcap", str(HELPER)], text=True).strip()
    assert capabilities == f"{HELPER} cap_net_admin,cap_net_raw=ep"
    expected = {
        "library_sha256": sha256(args.payload / "opt/shuvpn/lib/libshuvpn_core.so"),
        "helper_sha256": sha256(args.payload / "usr/lib/shuvpn/shuvpn_tun_helper"),
    }

    def verify_files():
        assert sha256(LIBRARY) == expected["library_sha256"]
        assert sha256(HELPER) == expected["helper_sha256"]
    verify_files()
    root = Path(__file__).resolve().parent.parent
    flutter = args.flutter.resolve()
    runtime = flutter / "bin/cache/dart-sdk/bin/dart"
    engine = flutter / "bin/cache/artifacts/engine/linux-x64/flutter_tester"
    for executable in (runtime, engine, Path("/opt/shuvpn/shuvpn")):
        assert not subprocess.check_output(["getcap", str(executable)], text=True).strip()
    certificate = report / "certificate.pem"
    key = report / "private-key.pem"
    subprocess.run([
        "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
        "-subj", "/CN=provider.fixture.test",
        "-addext", "basicConstraints=critical,CA:FALSE",
        "-addext", "extendedKeyUsage=serverAuth",
        "-addext", "subjectAltName=DNS:provider.fixture.test,IP:10.22.0.5",
        "-keyout", str(key), "-out", str(certificate),
    ], check=True, capture_output=True)
    certificate.chmod(0o644)
    peer = subprocess.Popen(["unshare", "--net", sys.executable, "-u", "-c", PEER,
                             str(certificate), str(key)],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True)
    child = None
    observed = []
    baseline = None
    summaries = []
    checkpoint = None
    coverage = False
    completed_stages = []
    engine_pid = None
    try:
        assert line(peer) == "PEER_READY"
        ip("link", "set", "lo", "up")
        ip("link", "add", "shuphys", "type", "veth", "peer", "name", "shupeer")
        ip("link", "set", "shupeer", "netns", str(peer.pid))
        ip("link", "set", "shuphys", "up")
        ip("address", "add", "192.0.2.1/24", "dev", "shuphys")
        ip("-6", "address", "add", "2001:db8:ffff::1/64", "dev", "shuphys", "nodad")
        peer.stdin.write("configure\n")
        peer.stdin.flush()
        ports = json.loads(line(peer))
        ip("route", "add", "default", "via", "192.0.2.2", "dev", "shuphys")
        ip("-6", "route", "add", "default", "via", "2001:db8:ffff::2", "dev", "shuphys")
        wait_addresses()
        baseline = routing_state()
        token = uuid.uuid4().hex
        config = {
            **expected, "uid": account.pw_uid, "token": token, "ports": ports,
            "inside_net": namespace("net"), "inside_mnt": namespace("mnt"),
            "parent_net": os.environ["SHUVPN_INSTALL_PARENT_NET"],
            "parent_mnt": os.environ["SHUVPN_INSTALL_PARENT_MNT"],
            "fixture": str(args.fixture.resolve()), "work": str(work),
            "report": str(work / "result.json"), "barriers": str(barriers),
            "certificate": str(certificate),
        }
        config_path = report / "config.json"
        config_path.write_text(json.dumps(config))
        config_path.chmod(0o644)
        environment = fixture_environment(dict(
            os.environ, SHUVPN_INSTALLED_FULL_CONFIG=str(config_path),
            FLUTTER_SUPPRESS_ANALYTICS="true", CI="true"))
        environment.pop("SHUVPN_LINUX_TUN_HELPER", None)
        child = subprocess.Popen([
            "runuser", "-u", args.account, "--", str(runtime),
            str(flutter / "bin/cache/flutter_tools.snapshot"),
            "test", "--no-pub", str(root / "tool/full_capture_adapter_smoke.dart"),
        ], cwd=root, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            start_new_session=True)
        pending = b""
        deadline = time.monotonic() + 150
        with (report / "flutter.log").open("wb") as log:
            while time.monotonic() < deadline:
                if not select.select([child.stdout], [], [], 0.2)[0]:
                    if child.poll() is not None:
                        break
                    continue
                chunk = os.read(child.stdout.fileno(), 65536)
                if not chunk:
                    break
                log.write(chunk)
                log.flush()
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
                pending += chunk
                while b"\n" in pending:
                    raw, pending = pending.split(b"\n", 1)
                    marker = b"INSTALLED_FULL_EVENT "
                    if marker not in raw:
                        continue
                    event = json.loads(raw.split(marker, 1)[1])
                    assert event["token"] == token
                    stage = validate_stage(completed_stages, event["stage"])
                    parent = event["pid"]
                    assert type(parent) is int and parent > 1
                    name = event["interface"]
                    if engine_pid is None:
                        engine_pid = parent
                    assert parent == engine_pid
                    if stage.endswith("-ready"):
                        assert name.startswith("shuvpn") and len(name) <= 15
                        pids = helper_pids()
                        assert len(pids) == 1, f"expected active fixed helper, got {pids}"
                        descriptor, summary = identity(pids.pop(), parent, account.pw_uid,
                                                       config["inside_net"])
                        observed.append(descriptor)
                        summaries.append({**summary, "stage": stage, "interface": name})
                        assert _library_in_maps(parent), "engine did not map the installed library"
                        parent_status = dict(row.split(":", 1) for row in
                                             Path(f"/proc/{parent}/status").read_text().splitlines())
                        assert list(map(int, parent_status["Uid"].split())) == [account.pw_uid] * 4
                        assert int(parent_status["CapEff"].strip(), 16) == 0
                        assert parent_status["NoNewPrivs"].strip() == "0"
                        assert os.readlink(f"/proc/{parent}/exe") == str(engine)
                        assert namespace("net", parent) == config["inside_net"]
                        assert namespace("mnt", parent) == config["inside_mnt"]
                        for family, destination in (("-4", "10.22.0.5"),
                                                    ("-6", "2001:db8:22::5")):
                            assert json.loads(ip("-j", family, "route", "get",
                                                 destination))[0]["dev"] == name
                    elif stage == "coverage-change":
                        assert name == summaries[-1]["interface"]
                        ip("route", "add", "198.51.100.33/32", "via", "192.0.2.2",
                           "dev", "shuphys")
                        coverage = True
                    elif stage == "helper-crash":
                        assert name == summaries[-1]["interface"]
                        assert observed and not exited(observed[-1])
                        signal.pidfd_send_signal(observed[-1], signal.SIGKILL)
                    elif stage in ("before-blocked", "after-blocked"):
                        peer.stdin.write("source-audit\n")
                        peer.stdin.flush()
                        audit = json.loads(line(peer))
                        attempts = [entry for entry in audit if entry[0] == "tcp"]
                        if stage == "before-blocked":
                            checkpoint = attempts
                        else:
                            assert checkpoint is not None and attempts == checkpoint, (
                                "rejected, unauthorized or ambiguous flow reached physical TCP")
                    elif stage.endswith("-cleanup"):
                        assert name == summaries[-1]["interface"]
                        _wait_exit(observed[-1])
                        assert not helper_pids(), "fixed helper or replacement survived cleanup"
                        assert subprocess.run(["/usr/sbin/ip", "link", "show", "dev", name],
                                              capture_output=True).returncode != 0
                        if coverage:
                            assert json.loads(ip("-j", "route", "show",
                                                 "198.51.100.33/32")), "foreign route was removed"
                            ip("route", "del", "198.51.100.33/32", "via", "192.0.2.2",
                               "dev", "shuphys")
                            coverage = False
                        check_routing(baseline)
                    else:
                        raise RuntimeError(f"unexpected stage: {stage}")
                    assert Path(f"/proc/{parent}").exists(), "engine exited before root confirmation"
                    acknowledge(barrier_directory, stage, token)
                    completed_stages.append(stage)
        child.wait(timeout=max(0.1, deadline - time.monotonic()))
        assert child.returncode == 0, f"Flutter probe exited {child.returncode}"
        result = json.loads((work / "result.json").read_text())
        assert result["full_direct_dual_stack_dns_https_reject_authorization_cleanup"]
        validate_complete(completed_stages)
        assert [fault["phase"] for fault in result["faults"]] == ["automatic", "manual-crash"]
        assert len(summaries) == 3
        peer.stdin.write("source-audit\n")
        peer.stdin.flush()
        sources = json.loads(line(peer))
        physical = {"192.0.2.1", "2001:db8:ffff::1"}
        assert all(address in physical for _, address in sources), sources
        required = {("udp", "192.0.2.1"), ("udp", "2001:db8:ffff::1"),
                    ("tcp", "192.0.2.1"), ("tcp", "2001:db8:ffff::1"),
                    ("dns-udp", "192.0.2.1"), ("dns-tcp", "192.0.2.1"),
                    ("https", "192.0.2.1")}
        assert required.issubset(map(tuple, sources)), sources
        assert not helper_pids() and all(exited(fd) for fd in observed)
        check_routing(baseline)
        verify_files()
        (report / "verification.json").write_text(json.dumps({
            "deb_sha256": sha256(args.deb), **expected, "helper_capabilities": capabilities,
            "probe_sha256": sha256(root / "tool/installed_full_capture_case.dart"),
            "fixture_sha256": sha256(args.fixture),
            "helper_observations": summaries, "sources": sources, "engine": result,
            "completed_stages": completed_stages,
            "routing_baseline": baseline, "routing_final": routing_state(),
            "ordinary_uid_cap_eff_zero": True, "fixed_helper_pidfd_exit_and_routes_restored": True,
            "rejected_flows_no_physical_tcp": True,
        }, indent=2) + "\n")
        print("INSTALLED_FULL_DEFAULT_CORE_FIXED_HELPER_DATA_FAULTS_AND_RECOVERY_PASSED", flush=True)
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
        peer.kill()
        peer.wait(timeout=5)
        if baseline is not None:
            ip("link", "del", "shuphys")
        for descriptor in observed:
            os.close(descriptor)
        os.close(barrier_directory)


def _library_in_maps(pid):
    return any(row.split()[-1:] == [str(LIBRARY)]
               for row in Path(f"/proc/{pid}/maps").read_text().splitlines())


def _wait_exit(descriptor):
    assert select.select([descriptor], [], [], 8)[0], "original helper identity did not exit"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--account", required=True)
    for name in ("flutter", "payload", "fixture", "deb", "report"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    run(parser.parse_args())
