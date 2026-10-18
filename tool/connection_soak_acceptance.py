"""Bounded local-protocol soak of one native desktop owner, with OS resource samples."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import socket
import subprocess
import sys
import threading
import time
import uuid

from connected_desktop_acceptance import echo, proxy_stream, receive_exact
from process_resources import ProcessResources

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ("10.21.0.5", "2001:db8:21::5")
BUDGETS = {"rss_bytes": 32 * 1024 * 1024, "private_bytes": 32 * 1024 * 1024,
           "vms_bytes": 128 * 1024 * 1024, "handles": 32, "fds": 32, "threads": 8}


def fixture_plan(value: dict, *, heartbeat_seconds: float = 0.05) -> dict:
    if type(heartbeat_seconds) not in (int, float) or heartbeat_seconds not in (0.05, 5.0):
        raise RuntimeError("Use the original 50 ms stress heartbeat or the separate 5 second auth-plan profile")
    nodes = value.get("nodes", {})
    if (value.get("sid") != "test-sid" or value.get("acceptAnyCertificate") is not False or
            not value.get("certificateDigests") or len(nodes) != 1 or
            value.get("heartbeatSeconds") != 0.05):
        raise RuntimeError("Expected the pinned, synthetic protocol fixture")
    addresses = list(nodes.values())[0]
    if len(addresses) != 1:
        raise RuntimeError("Expected one loopback fixture node")
    host, port = addresses[0].rsplit(":", 1)
    if host != "127.0.0.1" or not 0 < int(port) < 65536:
        raise RuntimeError("Fixture node is not a loopback TCP endpoint")
    routes = [route for route in value["routes"] if not route["host"].startswith("*.")]
    expected = {(host, protocol) for host in ("10.21.0.0/16", "2001:db8:21::/64")
                for protocol in ("tcp", "udp")}
    if len(routes) != 4 or {(route["host"], route["protocol"]) for route in routes} != expected:
        raise RuntimeError("Expected only synthetic IPv4/IPv6 fixture resources")
    return {**value, "routes": routes, "dnsServers": [], "heartbeatSeconds": heartbeat_seconds}


def heartbeat_traces(stderr: str) -> list[dict]:
    traces = []
    connections = set()
    for line in stderr.splitlines():
        if not line.startswith("FIXTURE_HEARTBEAT_TRACE "):
            continue
        try:
            trace = json.loads(line.removeprefix("FIXTURE_HEARTBEAT_TRACE "))
            counters = [trace["requests"], trace["responses"]]
            durations = [trace[name] for name in (
                "elapsed_seconds", "max_response_gap_seconds", "max_write_seconds",
                "last_response_age_seconds")]
            host, port = trace["peer"].rsplit(":", 1)
            events = trace["last_events"]
            connection = trace["connection"]
            if (type(connection) is not int or connection <= 0 or connection in connections or
                    host != "127.0.0.1" or not 0 < int(port) < 65536 or
                    any(type(value) is not int or value < 0 for value in counters) or
                    counters[0] - counters[1] not in (0, 1) or
                    type(trace["pending_request"]) is not bool or
                    trace["pending_request"] != (counters[0] != counters[1]) or
                    any(type(value) not in (int, float) or not math.isfinite(value) or value < 0
                        for value in durations) or
                    not isinstance(events, list) or len(events) > 64 or
                    (not events and counters[0] != 0)):
                raise ValueError("Invalid heartbeat trace fields")
            previous = [0, 0, 0.0]
            for event in events:
                current = [event["requests"], event["responses"], event["elapsed_seconds"]]
                name = event["event"]
                if (not isinstance(name, str) or
                        name not in ("request_received", "response_flushed") and
                        not name.startswith("read_end: ") or
                        any(type(value) is not int or value < previous[index] or value > counters[index]
                            for index, value in enumerate(current[:2])) or
                        current[0] - current[1] not in (0, 1) or
                        type(current[2]) not in (int, float) or not math.isfinite(current[2]) or
                        not previous[2] <= current[2] <= trace["elapsed_seconds"]):
                    raise ValueError("Unordered or invalid retained heartbeat events")
                previous = current
            if events and previous[:2] != counters:
                raise ValueError("Retained heartbeat tail differs from its cumulative counters")
            connections.add(connection)
        except (ValueError, TypeError, KeyError, AttributeError) as error:
            raise RuntimeError("Invalid diagnostic fixture heartbeat trace") from error
        traces.append(trace)
    return traces


def validate_native(value: dict, *, connected: bool, closed: bool = False) -> None:
    owner = value.get("owner") or {}
    session = value.get("session") or {}
    if (owner.get("cleanup_pending") is not False or owner.get("closed") is not closed or
            owner.get("phase") != ("connected" if connected else "disconnected") or
            (connected and not owner.get("run_id")) or
            (not connected and owner.get("run_id") is not None) or
            owner.get("error_code") is not None or owner.get("error_message") is not None):
        raise RuntimeError("Owner did not reach the required lifecycle barrier")
    if session:
        if (session.get("state") != ("connected" if connected else "disconnected") or
                session.get("tun") != {"state": "stopped", "cleanup_pending": False}):
            raise RuntimeError("Unexpected session state or TUN activity")
        egress = session.get("egress") or {}
        if not connected and (egress.get("active") is not False or
                              egress.get("pending_cleanup") is not False or egress.get("sockets") != []):
            raise RuntimeError("Disconnected session retained native egress resources")
    elif connected or value.get("command") == "disconnect":
        raise RuntimeError("Connection barrier has no session snapshot")
    if connected:
        ports = value.get("ports") or {}
        if set(ports) != {"http", "socks"} or any(
                type(port) is not int or not 0 < port < 65536 for port in ports.values()):
            raise RuntimeError("Expected both actual ephemeral proxy bindings")


def validate_traffic(value: dict, flows: list[dict]) -> None:
    validate_native(value, connected=True)
    session = value["session"]
    uploaded = sum(flow["payload_bytes"] for flow in flows)
    if (session["upload"] != uploaded or session["download"] != uploaded + 7 * len(flows) or
            session["connections"] != len(flows)):
        raise RuntimeError("Native byte/connection totals differ from independently verified flows")
    for flow in flows:
        if not any(row["flow"]["inbound"] == flow["protocol"] and
                   row["flow"]["host"] == flow["address"] and row["flow"]["port"] == 443 and
                   row["flow"]["network"] == "tcp" and row.get("outbound") == "SHU-VPN" and
                   row["decision"]["action"] == "SHU-VPN"
                   for row in session["decisions"]):
            raise RuntimeError("Native decisions lack the independently exercised outbound")


def growth(baseline: dict, current: dict) -> dict:
    if set(baseline) != set(current) or not baseline:
        raise RuntimeError("Resource counter set changed")
    changes = {}
    for name, initial in baseline.items():
        value = current[name]
        if (name not in BUDGETS or type(initial) is not int or type(value) is not int or
                initial <= 0 or value <= 0):
            raise RuntimeError("Missing or invalid OS resource sample")
        changes[name] = value - initial
        if changes[name] > BUDGETS[name]:
            raise RuntimeError(f"Resource growth exceeded the predeclared {name} budget: {changes[name]}")
    return changes


class JsonProcess:
    def __init__(self, command: list[str], report: Path, name: str, *, env=None):
        self.error_log = (report / f"{name}-stderr.log").open("w", encoding="utf-8")
        self.output_log = (report / f"{name}-stdout.jsonl").open("w", encoding="utf-8")
        try:
            self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                            stderr=self.error_log, text=True, encoding="utf-8", env=env)
        except BaseException:
            self.error_log.close()
            self.output_log.close()
            raise
        self.messages = queue.Queue()
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()

    def _read(self):
        try:
            for line in self.process.stdout:
                self.output_log.write(line)
                self.output_log.flush()
                self.messages.put(line)
        finally:
            self.messages.put(None)

    def receive(self) -> dict:
        line = self.messages.get(timeout=20)
        if line is None:
            raise RuntimeError("Owned child exited before its expected response")
        return json.loads(line)

    def close(self) -> dict:
        forced = False
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                forced = True
                self.process.kill()
                self.process.wait(timeout=5)
        self.reader.join(timeout=3)
        if self.reader.is_alive():
            raise RuntimeError("Child stdout reader did not terminate")
        self.process.stdout.close()
        if not self.process.stdin.closed:
            self.process.stdin.close()
        self.error_log.close()
        self.output_log.close()
        return {"pid": self.process.pid, "exit": self.process.returncode, "forced": forced}


class Probe:
    def __init__(self, child: JsonProcess, token: str):
        self.child, self.token, self.sequence = child, token, 0

    def validate(self, value: dict) -> dict:
        if value.get("token") != self.token or value.get("pid") != self.child.process.pid:
            raise RuntimeError("Probe response belongs to another child/token")
        return value

    def call(self, command: str) -> dict:
        self.sequence += 1
        self.child.process.stdin.write(json.dumps({
            "id": self.sequence, "token": self.token, "command": command}) + "\n")
        self.child.process.stdin.flush()
        value = self.validate(self.child.receive())
        if value.get("id") != self.sequence or value.get("command") != command:
            raise RuntimeError("Probe response sequence/command mismatch")
        return value


def listener_barrier(observer: ProcessResources, ports: dict) -> dict:
    deadline = time.monotonic() + 3
    while True:
        if not observer.alive():
            raise RuntimeError("Listener observation requires the original process still alive")
        matches = []
        if sys.platform == "win32":
            tables = observer.desktop.listeners()
            matches = [row for row in tables["matches"] if row["pid"] == observer.pid]
        else:
            sockets = set()
            for path in Path(f"/proc/{observer.pid}/fd").iterdir():
                try:
                    target = os.readlink(path)
                except FileNotFoundError:
                    continue
                if target.startswith("socket:["):
                    sockets.add(target[8:-1])
            for family in ("tcp", "tcp6"):
                for line in Path(f"/proc/{observer.pid}/net/{family}").read_text().splitlines()[1:]:
                    fields = line.split()
                    if fields[3] == "0A" and fields[9] in sockets:
                        matches.append({"family": family, "port": int(fields[1].rsplit(":", 1)[1], 16)})
        if not observer.alive():
            raise RuntimeError("Process exited during listener observation")
        if not matches:
            return {"owned_listeners": matches, "old_ports": ports,
                    "method": "GetExtendedTcpTable" if sys.platform == "win32" else "proc-tcp-tcp6-owned-inodes"}
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Owned listener remained after disconnect: {matches}")
        time.sleep(0.05)


def held_flow(port: int, protocol: str, address: str, seconds: int,
              stop: threading.Event, report: Path) -> dict:
    started = time.monotonic()
    count = 0
    payload = bytes(range(256)) * 256
    error_message = None
    half_close_checked = False
    path = report / f"held-{protocol}-{'v6' if ':' in address else 'v4'}.json"

    def progress():
        path.write_text(json.dumps({
            "protocol": protocol, "address": address, "completed_batches": count,
            "verified_payload_bytes": count * len(payload),
            "elapsed_seconds": time.monotonic() - started, "error": error_message,
            "requested_seconds": seconds,
            "duration_complete": time.monotonic() - started >= seconds,
            "half_close_eof": half_close_checked,
        }, indent=2), encoding="utf-8")

    try:
        with proxy_stream(port, protocol, address) as stream:
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline and not stop.is_set():
                stream.sendall(payload)
                if receive_exact(stream, len(payload)) != payload:
                    raise RuntimeError("Held connection returned corrupt payload")
                count += 1
                if count % 10 == 0:
                    progress()
                stop.wait(0.5)
            stream.shutdown(socket.SHUT_WR)
            if stream.recv(1) != b"":
                raise RuntimeError("Held connection did not propagate half-close EOF")
            half_close_checked = True
    except Exception as error:
        error_message = f"{type(error).__name__}: {error}"
        raise RuntimeError(
            f"Held {protocol} {address} after {count} verified batches "
            f"at {time.monotonic() - started:.3f}s: {error}") from error
    finally:
        progress()
    elapsed = time.monotonic() - started
    if elapsed < seconds or not count:
        raise RuntimeError("Held flow ended before the configured duration")
    return {"protocol": protocol, "address": address, "payload_bytes": count * len(payload),
            "batches": count, "elapsed_seconds": elapsed, "prelude": True, "half_close_eof": True}


def run(args) -> None:
    heartbeat_seconds = getattr(args, "heartbeat_seconds", 0.05)
    trace_heartbeats = getattr(args, "trace_heartbeats", False)
    report = args.report.resolve()
    if report == (ROOT / "build/desktop").resolve() or not report.is_relative_to((ROOT / "build/desktop").resolve()):
        raise RuntimeError("Soak report must be a new directory below build/desktop")
    for name in ("probe", "fixture", "library"):
        setattr(args, name, getattr(args, name).resolve(strict=True))
    report.mkdir(parents=True)
    products = {name: hashlib.sha256(getattr(args, name).read_bytes()).hexdigest()
                for name in ("probe", "fixture", "library")}
    config = {"hold_seconds": args.hold_seconds, "cycles": args.cycles, "warmup": args.warmup,
              "heartbeat_seconds": heartbeat_seconds, "trace_heartbeats": trace_heartbeats,
              "products": products,
              "growth_budgets": BUDGETS, "scope": "synthetic pinned loopback plan; one native owner; no TUN/system proxy"}
    (report / "config.json").write_text(json.dumps(config, indent=2), encoding="utf-8")
    children, observers, exits, identities = [], [], [], {}
    started, failure = time.monotonic(), None
    samples, rounds, run_ids = [], [], set()
    baseline = None
    final = None
    probe = None
    failure_native = None
    failure_capture_error = None
    cleanup_errors = []
    token = uuid.uuid4().hex
    try:
        fixture_environment = dict(os.environ)
        fixture_environment.pop("SHUVPN_FIXTURE_HEARTBEAT_TRACE", None)
        if trace_heartbeats:
            fixture_environment["SHUVPN_FIXTURE_HEARTBEAT_TRACE"] = "1"
        fixture = JsonProcess([str(args.fixture), "udp"], report, "fixture", env=fixture_environment)
        children.append(fixture)
        plan = fixture_plan(fixture.receive(), heartbeat_seconds=heartbeat_seconds)
        plan_file = report / "plan.json"
        plan_file.write_text(json.dumps(plan), encoding="utf-8")
        child = JsonProcess([str(args.probe), str(args.library), str(report / "rules"),
                             str(plan_file), token], report, "probe",
                            env=dict(os.environ, SHUVPN_SOAK_TOKEN=token))
        children.append(child)
        probe = Probe(child, token)
        ready = probe.validate(child.receive())
        if ready.get("ready") is not True:
            raise RuntimeError("Probe did not report ready")
        validate_native(ready, connected=False)
        for name, process, executable in (("probe", child, args.probe), ("fixture", fixture, args.fixture)):
            observer = ProcessResources(process.process.pid, executable)
            observers.append(observer)
            identities[name] = observer.identity
            if observer.identity["parent_pid"] != os.getpid():
                raise RuntimeError("Observed process is not this driver's original child")
        native_process, fixture_process = observers
        native_process.require_library(args.library)

        def connect():
            value = probe.call("connect")
            validate_native(value, connected=True)
            identity = value["owner"]["run_id"]
            if identity in run_ids:
                raise RuntimeError("A reconnect reused the previous session run ID")
            run_ids.add(identity)
            return value

        def sample(stage, round_number):
            values = {"probe": native_process.sample(), "fixture": fixture_process.sample()}
            value = {"stage": stage, "round": round_number,
                     "elapsed_seconds": time.monotonic() - started, "resources": values,
                     "growth_since_warmup": None}
            try:
                if baseline is not None:
                    value["growth_since_warmup"] = {
                        name: growth(baseline[name], values[name]) for name in values}
            except RuntimeError as error:
                value["growth_error"] = str(error)
                raise
            finally:
                samples.append(value)
                with (report / "resources.jsonl").open("a", encoding="utf-8") as output:
                    output.write(json.dumps(value) + "\n")
            return values

        def disconnect(value, flows):
            # Traffic counters are asynchronous; request snapshots until complete.
            deadline = time.monotonic() + 5
            while True:
                current = probe.call("snapshot")
                try:
                    validate_traffic(current, flows)
                    break
                except RuntimeError:
                    if time.monotonic() >= deadline:
                        raise
                    time.sleep(0.05)
            stopped = probe.call("disconnect")
            validate_native(stopped, connected=False)
            barrier = listener_barrier(native_process, value["ports"])
            rounds.append({"round": value["round"], "run_id": value["owner"]["run_id"],
                           "flows": flows, "traffic": current["session"],
                           "disconnected": stopped, "listener_barrier": barrier})
            return sample("disconnected", value["round"])

        def cycle():
            value = connect()
            flows = [echo(value["ports"][protocol], protocol, target)
                     for protocol in ("http", "socks") for target in TARGETS]
            return disconnect(value, flows)

        for _ in range(args.warmup):
            baseline = None
            values = cycle()
        baseline = values
        (report / "baseline.json").write_text(json.dumps(baseline, indent=2), encoding="utf-8")
        value = connect()
        stop = threading.Event()
        try:
            with ThreadPoolExecutor(max_workers=4) as pool:
                tasks = [pool.submit(held_flow, value["ports"][protocol], protocol, target,
                                     args.hold_seconds, stop, report)
                         for protocol in ("http", "socks") for target in TARGETS]
                try:
                    while not all(task.done() for task in tasks):
                        for task in tasks:
                            if task.done() and task.exception() is not None:
                                raise task.exception()
                        validate_native(probe.call("snapshot"), connected=True)
                        sample("held-connected", value["round"])
                        print(f"SOAK_HELD {time.monotonic() - started:.1f}s", flush=True)
                        time.sleep(1)
                    flows = [task.result() for task in tasks]
                finally:
                    stop.set()
        finally:
            stop.set()
        disconnect(value, flows)
        for index in range(args.cycles):
            cycle()
            print(f"SOAK_RECONNECT {index + 1}/{args.cycles}", flush=True)
        final = probe.call("shutdown")
        validate_native(final, connected=False, closed=True)
        child.process.wait(timeout=10)
        if child.process.returncode != 0:
            raise RuntimeError("Native probe did not exit normally after shutdown")
        if len(rounds) != args.warmup + 1 + args.cycles:
            raise RuntimeError("The requested number of complete rounds was not observed")
    except BaseException as error:
        failure = error
        if probe is not None and probe.child.process.poll() is None:
            try:
                failure_native = probe.call("snapshot")
            except Exception as diagnostic_error:
                failure_capture_error = f"{type(diagnostic_error).__name__}: {diagnostic_error}"
    finally:
        for process in reversed(children):
            try:
                exits.append(process.close())
            except Exception as error:
                cleanup_errors.append(f"child cleanup: {error}")
        for observer in observers:
            try:
                if observer.alive():
                    raise RuntimeError("An original child survived cleanup")
            except Exception as error:
                cleanup_errors.append(f"retained process cleanup: {error}")
            finally:
                try:
                    observer.close()
                except Exception as error:
                    cleanup_errors.append(f"observer close: {error}")
    traces, trace_capture_error = [], None
    fixture_stderr = ""
    try:
        if children:
            fixture_stderr = (report / "fixture-stderr.log").read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        trace_capture_error = f"{type(error).__name__}: {error}"
        if failure is None:
            failure = error
    fixture_errors = [line.removeprefix("FIXTURE_CONNECTION_ERROR ") for line in fixture_stderr.splitlines()
                      if line.startswith("FIXTURE_CONNECTION_ERROR ")]
    if fixture_errors and failure is None:
        failure = RuntimeError(f"Diagnostic fixture reported connection errors: {fixture_errors}")
    if trace_heartbeats and trace_capture_error is None:
        try:
            traces = heartbeat_traces(fixture_stderr)
            if not traces or (failure is None and (
                    {trace["connection"] for trace in traces} != set(range(1, len(rounds) + 1)) or
                    not any(trace["requests"] for trace in traces))):
                raise RuntimeError("Requested fixture heartbeat observations were not complete")
        except RuntimeError as error:
            trace_capture_error = str(error)
            if failure is None:
                failure = error
    products_after = {}
    for name in products:
        try:
            products_after[name] = hashlib.sha256(getattr(args, name).read_bytes()).hexdigest()
        except OSError as error:
            cleanup_errors.append(f"product recheck {name}: {error}")
    summary = {"passed": failure is None and not cleanup_errors and products == products_after and all(
        row["exit"] == 0 and not row["forced"] for row in exits),
        "goal_complete": False, "config": config, "identities": identities,
        "elapsed_seconds": time.monotonic() - started, "baseline": baseline,
        "rounds": rounds, "samples": samples, "exits": exits, "final": final,
        "products": products, "products_after": products_after, "cleanup_errors": cleanup_errors,
        "failure_native": failure_native, "failure_capture_error": failure_capture_error,
        "fixture_heartbeat_traces": traces, "trace_capture_error": trace_capture_error,
        "fixture_connection_errors": fixture_errors,
        "error": None if failure is None else f"{type(failure).__name__}: {failure}"}
    (report / "verification.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if failure is not None:
        raise failure
    if not summary["passed"]:
        raise RuntimeError("Soak child exit/cleanup was incomplete")
    print("LOCAL_NATIVE_OWNER_SOAK_PASSED", flush=True)


def positive(value):
    number = int(value)
    if not 1 <= number <= 86400:
        raise argparse.ArgumentTypeError("Expected an integer from 1 to 86400")
    return number


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("probe", "fixture", "library", "report"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--hold-seconds", type=positive, default=600)
    parser.add_argument("--cycles", type=positive, default=100)
    parser.add_argument("--warmup", type=positive, default=10)
    parser.add_argument("--heartbeat-seconds", type=float, choices=(0.05, 5.0), default=0.05,
                        help="Separate fixture stress (0.05) and auth-plan timing (5.0); neither authenticates an account")
    parser.add_argument("--trace-heartbeats", action="store_true",
                        help="Ask the diagnostic fixture to retain its last 64 control-heartbeat events")
    run(parser.parse_args())
