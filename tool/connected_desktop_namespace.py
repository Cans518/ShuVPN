"""Run only the connected fixture GUI inside a fresh loopback-only Linux net namespace."""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import sys


def network():
    return os.readlink("/proc/self/ns/net")


def snapshot():
    commands = [["ip", "-j", "link", "show"],
                *[["ip", "-j", family, kind, "show"] + (["table", "all"] if kind == "route" else [])
                  for family in ("-4", "-6") for kind in ("route", "rule")]]
    return [json.loads(subprocess.check_output(command, text=True)) for command in commands]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("bundle", "report", "dart", "fixture"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--user", required=True)
    parser.add_argument("--inside", action="store_true")
    args = parser.parse_args()
    if sys.platform != "linux" or os.geteuid() != 0:
        raise RuntimeError("Namespace preparation requires Linux root; the GUI runs as an ordinary user")
    user = pwd.getpwnam(args.user)
    if user.pw_uid == 0:
        raise RuntimeError("The GUI account must be an ordinary user")
    if not args.inside:
        baseline = snapshot()
        environment = dict(os.environ, SHUVPN_CONNECTED_PARENT_NET=network())
        result = subprocess.run(["unshare", "--net", sys.executable, str(Path(__file__).resolve()),
                                 *sys.argv[1:], "--inside"], env=environment, check=False)
        final = snapshot()
        record = {"exit": result.returncode, "parent_net": network(),
                  "host_before": baseline, "host_after": final, "host_unchanged": baseline == final}
        path = args.report.with_name(args.report.name + "-namespace.json")
        path.write_text(json.dumps(record, indent=2) + "\n")
        if result.returncode or baseline != final:
            raise RuntimeError("Connected GUI failed or host network changed")
        return
    parent = os.environ.get("SHUVPN_CONNECTED_PARENT_NET")
    if not parent or parent == network():
        raise RuntimeError("A fresh private network namespace is required")
    interfaces = json.loads(subprocess.check_output(["ip", "-j", "link", "show"], text=True))
    if [row["ifname"] for row in interfaces] != ["lo"]:
        raise RuntimeError("Connected GUI namespace must contain loopback only")
    subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
    baseline = snapshot()
    environment = {key: value for key, value in os.environ.items()
                   if key.lower() not in ("http_proxy", "https_proxy", "all_proxy", "no_proxy")}
    environment.update(HOME=user.pw_dir, USER=user.pw_name, LOGNAME=user.pw_name,
                       SHUVPN_CONNECTED_GUI_FIXTURE=str(args.fixture.resolve()))
    result = subprocess.run([
        "runuser", "-u", args.user, "--", "/usr/bin/python3",
        str(Path(__file__).with_name("connected_desktop_acceptance.py")),
        "--bundle", str(args.bundle.resolve()), "--report", str(args.report.resolve()),
        "--dart", str(args.dart.resolve()),
    ], env=environment, check=False)
    final = snapshot()
    if baseline != final or result.returncode:
        raise RuntimeError("Private connected GUI failed or changed namespace network")
    (args.report / "namespace.json").write_text(json.dumps({
        "parent_net": parent, "inside_net": network(), "ordinary_uid": user.pw_uid,
        "before": baseline, "after": final, "private_network_unchanged": True,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
