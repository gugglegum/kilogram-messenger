"""Coordinate a reviewed Windows LAN terminal pilot over pinned SSH."""
import argparse
import base64
from contextlib import ExitStack
import hashlib
import ipaddress
import json
from pathlib import Path, PureWindowsPath
import queue
import re
import subprocess
import sys
import threading
import time
import uuid

SYSTEM = Path("C:/Windows/System32")
IMAGES = ("kilogram-cli.exe", "kilogram-bootstrap.exe", "t11f-fixture.exe", "host.ps1", "deadline.ps1")
OUT_CAP = 256 * 1024
HEX_ID = re.compile("[0-9a-f]{64}")
PENDING = {"queued", "materialized"}
INCOMING_FIELDS = ("context", "runtime_instance", "epoch", "event_id", "text")


def require(value, message):
    if not value:
        raise RuntimeError(message)
    return value


def save(path, value):
    with path.open("x", encoding="utf-8") as out:
        json.dump(value, out, indent=2, ensure_ascii=False)
        out.write("\n")


def remaining(deadline, cap):
    seconds = min(cap, deadline - time.monotonic())
    return require(seconds if seconds > 0 else None, "pilot deadline")


def until(action, timeout, deadline, label):
    end = min(time.monotonic() + timeout, deadline)
    while time.monotonic() < end:
        if result := action(end):
            return result
        time.sleep(min(0.75, max(0, end - time.monotonic())))
    raise RuntimeError(label)


def powershell_literal(value):
    return "'" + value.replace("'", "''") + "'"


def pinned_ssh(config, local_ip, peer_ip):
    known = Path(config["known_hosts"])
    host = str(peer_ip)
    pin = subprocess.check_output(
        [SYSTEM / "OpenSSH/ssh-keygen.exe", "-F", host, "-lf", known], timeout=5, text=True
    )
    require([line.split() for line in pin.splitlines() if not line.startswith("#")]
            == [[host, "ED25519", config["fingerprint"]]], "expected prior Ed25519 host pin")
    require(re.fullmatch(r"[\w.-]+", config["user"], re.ASCII), "invalid SSH user")
    return [str(SYSTEM / "OpenSSH/ssh.exe"), "-F", "NUL", "-T",
            "-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes", "-o", "StrictHostKeyChecking=yes",
            "-o", "HostKeyAlgorithms=ssh-ed25519", "-o", "UpdateHostKeys=no",
            "-o", "GlobalKnownHostsFile=NUL", "-o", f"UserKnownHostsFile={known.as_posix()}",
            "-o", f"BindAddress={local_ip}", "-o", "ClearAllForwardings=yes",
            "-o", "ConnectTimeout=5", "-o", "ConnectionAttempts=1",
            "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=2",
            "-i", config["identity"], f"{config['user']}@{host}"]


class Host:
    def __init__(self, name, config, peer, attempt, pins, ssh, evidence, deadline):
        self.name = name
        self.base = PureWindowsPath(config["base"])
        require(self.base.is_absolute() and ".." not in self.base.parts, "absolute dedicated base required")
        self.setup = self.base / "runs" / attempt / "setup"
        self.udp = config.get("udp_ephemeral")
        self.config = {"base": str(self.base), "ip": config["ipv4"], "hostname": config["hostname"],
                       "peer": peer, "attempt": attempt, "pins": pins, "ssh_peer": peer if ssh else None}
        self.ps = (ssh + ["powershell.exe"]) if ssh else [str(SYSTEM / "WindowsPowerShell/v1.0/powershell.exe")]
        self.evidence = evidence
        self.deadline = deadline

    def command(self, op, **args):
        request = json.dumps(self.config | {"op": op} | args, ensure_ascii=True, separators=(",", ":"))
        script = powershell_literal(str(self.base / "bin" / "host.ps1"))
        pin = self.config["pins"]["host.ps1"]
        code = (
            "$ErrorActionPreference='Stop'; $p=" + script + ";"
            'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Management/Microsoft.PowerShell.Management.psd1";'
            'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";'
            f"if((Get-FileHash -LiteralPath $p).Hash -ine '{pin}'){{throw 'host script SHA-256 mismatch'}};"
            "& ([scriptblock]::Create([IO.File]::ReadAllText($p))) -Request " + powershell_literal(request)
        )
        command = base64.b64encode(code.encode("utf-16-le")).decode()
        require(len(command) < 7500, "encoded Windows command length")
        return self.ps + ["-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", command]

    def call(self, op, data=b"", timeout=25, cleanup=False, **args):
        limit = timeout if cleanup else remaining(self.deadline, timeout)
        result = subprocess.run(self.command(op, **args), input=data, capture_output=True, timeout=limit)
        require(result.returncode == 0 and not result.stderr and len(result.stdout) <= 700000,
                f"{self.name}/{op}: exit={result.returncode}, stderr={result.stderr[:2048]!r}")
        return json.loads(result.stdout.decode("utf-8-sig"))

    def receipt(self, tag, *, descriptor, cleanup=False):
        result = self.call("receipt", tag=tag, cleanup=cleanup)
        save(self.evidence / f"{self.name}-{tag}-job.json", result)
        require(result["tag"] == tag and result["host"].casefold() == self.config["hostname"].casefold()
                and result["child_exit"] == 0 and result["membership"] and not result["forced"]
                and not result["before"] and not result["after"] and result["error"] is None
                and (descriptor is None or result["descriptor"] == descriptor),
                f"{self.name}/{tag}: unconfirmed child shutdown")

    def socket(self, tag, end):
        def check(_):
            row = self.call("socket", tag=tag)
            return row if row["observed"] else None
        row = until(check, 20, end, f"{self.name}: no owned UDP socket receipt")
        require(row["ip"] == self.config["ip"] and self.udp[0] <= row["port"] <= self.udp[1],
                f"{self.name}: unexpected LAN UDP socket")
        path = self.evidence / f"{self.name}-{tag}-socket.json"
        if not path.exists():
            save(path, row)

    def fixture(self, action, input_name=None):
        tag = "fixture-" + action
        try:
            result = self.call("fixture", tag=tag, action=action, input=input_name, timeout=108)
        except Exception as failure:
            try:
                self.receipt(tag, descriptor=False, cleanup=True)
            except Exception as receipt_failure:
                failure.add_note(str(receipt_failure))
            raise
        else:
            self.receipt(tag, descriptor=False, cleanup=True)
            return result


def transfer(source, dest, path, name, ledger):
    path = PureWindowsPath(path.removeprefix("\\\\?\\"))
    require(path.parent in {source.setup / "exchange", source.setup / "connection"}, "unexpected export path")
    wire = source.call("get", name=path.name)
    raw = base64.b64decode(wire["data"], validate=True)
    digest = hashlib.sha256(raw).hexdigest()
    require(digest == wire["sha256"], "source transfer SHA-256 mismatch")
    kind = "locators" if name.endswith(".json") else Path(name).stem
    require(json.loads(raw).get("kind") == kind, "unexpected public artifact type")
    receipt = dest.call("put", name=name, sha256=digest, data=wire["data"].encode("ascii") + b"\n")
    require(receipt["sha256"] == digest, "destination transfer SHA-256 mismatch")
    ledger.append({"source": source.name, "destination": dest.name, "kind": name,
                   "sha256": digest, "bytes": len(raw)})
    return receipt["path"]


class Terminal:
    def __init__(self, argv, deadline):
        self.deadline = deadline
        self.rows = queue.Queue()
        self.events = []
        self.errors = bytearray()
        self.last_readiness = None
        self.live_readiness = None
        self.quit_verified = False
        self.seq = 0
        self.proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.readers = [threading.Thread(target=fn, daemon=True) for fn in (self.read, self.read_errors)]
        for reader in self.readers:
            reader.start()

    def read(self):
        try:
            while raw := self.proc.stdout.readline(OUT_CAP + 1):
                require(len(raw) <= OUT_CAP and raw.endswith(b"\n"), "invalid terminal line bound")
                doc = json.loads(raw)
                require(doc["v"] == 1, "unexpected JSONL version")
                if doc.get("type") == "event":
                    require(doc.get("event") == "incoming" and "id" not in doc
                            and HEX_ID.fullmatch(doc["event_id"]) and len(self.events) < 32,
                            "unexpected terminal event or resync")
                    self.events.append({field: doc[field] for field in INCOMING_FIELDS})
                else:
                    self.rows.put(doc)
        except Exception as exc:
            self.rows.put(exc)
        finally:
            self.rows.put(None)

    def read_errors(self):
        while raw := self.proc.stderr.read(4096):
            self.errors.extend(raw[:max(0, 32768 - len(self.errors))])

    def line(self, end):
        row = self.rows.get(timeout=remaining(end, 55))
        if isinstance(row, Exception):
            raise row
        return row

    def result(self, op, args, end):
        self.seq += 1
        tag = f"t11f-{self.seq}"
        payload = json.dumps({"v": 1, "id": tag, "op": op, "args": args}, ensure_ascii=False)
        self.proc.stdin.write(payload.encode("utf-8") + b"\n")
        self.proc.stdin.flush()
        response = self.line(end)
        require(response and response["type"] == "result" and response["id"] == tag, f"{op}: correlation or EOF")
        return response

    def call(self, op, end=None, **args):
        response = self.result(op, args, end or self.deadline)
        require(response["ok"], f"{op}: {response.get('code')}")
        return response["value"]

    def status(self, end):
        def attempt(short_end):
            response = self.result("status", {}, short_end)
            if response.get("code") == "session_closing":
                return None
            require(response["ok"] and "monitor" in response, f"status: {response.get('code')}")
            return response["value"]["status"], response["monitor"]
        return until(attempt, 8, end, "terminal still closing")

    def connect(self, instance=None):
        require(self.line(self.deadline)["type"] == "ready", "missing terminal ready")
        args = {"mode": "start"} if instance is None else {"mode": "attach", "runtime_instance": instance}
        binding = self.call("connect", **args)
        self.contexts = binding["contexts"]
        self.instance = binding["runtime_instance"]
        require(HEX_ID.fullmatch(self.instance) and len(set(self.contexts)) == 2
                and all(HEX_ID.fullmatch(ctx) for ctx in self.contexts), "invalid connection identity")
        if instance:
            require(self.instance == instance, "observer attached to a different runtime")

    def ready(self, end, live):
        status, monitor = self.status(end)
        self.last_readiness = {"runtime_instance": status["runtime_instance"],
                               "live_admitted": status["live_admitted"], "monitor": monitor}
        require(status["runtime_instance"] == self.instance, "local runtime instance changed")
        if (live and not status["live_admitted"]) or not monitor["epoch"]:
            return None
        if [(r["context"], r["state"]) for r in monitor["contexts"]] != [(ctx, "watching") for ctx in self.contexts]:
            return None
        self.epoch = monitor["epoch"]
        if live:
            self.live_readiness = self.last_readiness
        return True

    def close(self):
        if not self.proc.stdin.closed:
            self.proc.stdin.close()
        forced = False
        try:
            self.proc.wait(timeout=max(0.1, min(35, self.deadline + 50 - time.monotonic())))
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=5)
            forced = True
        for reader in self.readers:
            reader.join(3)
        return not forced and not any(reader.is_alive() for reader in self.readers)

    def quit(self):
        def attempt(end):
            result = self.result("quit", {}, end)
            if result.get("code") == "session_closing":
                return None
            require(result["ok"] and result["value"] == {"connected": False}, "quit result")
            return True
        until(attempt, 10, self.deadline, "quit blocked by session_closing")
        require(self.close() and self.proc.returncode == 0 and not self.errors, "terminal exit or streams")
        require(self.rows.get(timeout=1) is None, "terminal stdout EOF")
        self.quit_verified = True


def history(terminal, context, end=None):
    page = terminal.call("history", end, context=context, limit=16)
    require(page["context"] == context and page["finished"], "fixture history must fit one page")
    return page["items"]


def agreement(sides, slot, event_id, text):
    rows = []
    for host, terminal in sides:
        found = [r for r in history(terminal, terminal.contexts[slot]) if r["event_id"] == event_id]
        require(len(found) == 1 and found[0]["text"] == text, f"{host.name}: history differs from receipt")
        rows.append(found[0])
    require(rows[0] == rows[1], "history role, ordinal or content differ")


def verify_final_history(snapshots, messages):
    expected_contexts = {message["key"]["context"] for message in messages}
    for snapshot in snapshots:
        context_ids = [context["context"] for context in snapshot["contexts"]]
        require(len(context_ids) == len(expected_contexts) and set(context_ids) == expected_contexts,
                f"{snapshot['host']}: final history contexts differ from deliveries")
    for slot, context in enumerate(snapshots[0]["contexts"]):
        context_id = context["context"]
        expected_rows = [message for message in messages if message["key"]["context"] == context_id]
        expected = {message["event_id"]: message["text"] for message in expected_rows}
        require(len(expected) == len(expected_rows), f"{context_id}: duplicate delivery ID")
        peer = snapshots[1]["contexts"][slot]
        require(peer["context"] == context_id, "terminal history context differs")
        left, right = ({row["event_id"]: row for row in entry["rows"]} for entry in (context, peer))
        for snapshot, entry, rows in zip(snapshots, (context, peer), (left, right)):
            require(len(entry["rows"]) == len(rows), f"{snapshot['host']}/{context_id}: duplicate history ID")
            require({event_id: row["text"] for event_id, row in rows.items()} == expected,
                    f"{snapshot['host']}/{context_id}: final history differs from deliveries")
        require(left == right, f"{context_id}: final history rows differ between hosts")


def prepare_fixtures(hosts, transfers):
    a, b = hosts
    for host in hosts:
        save(host.evidence / f"{host.name}-init.json", host.call("init"))
    invite = a.fixture("invite")["artifact"]
    transfer(a, b, invite, "invite.kilogram", transfers)
    joined = b.fixture("join", "invite.kilogram")
    transfer(b, a, joined["artifact"], "reply.kilogram", transfers)
    accepted = a.fixture("reply", "reply.kilogram")
    roles = [accepted["role"], joined["role"]]
    require(sorted(roles) == [0, 1], "canonical consent roles")
    proposer, cosigner = hosts[roles.index(0)], hosts[roles.index(1)]
    transfer(proposer, cosigner, proposer.fixture("propose")["artifact"], "proposal.kilogram", transfers)
    final = cosigner.fixture("cosign", "proposal.kilogram")["artifact"]
    transfer(cosigner, proposer, final, "final.kilogram", transfers)
    fixtures = [host.fixture("import", "final.kilogram" if host is proposer else None) for host in hosts]
    require(fixtures[0]["contexts"] == fixtures[1]["contexts"] and fixtures[0]["account"] != fixtures[1]["account"],
            "fixture contexts or identities")
    return fixtures


def run(config):
    attempt = config["attempt"]
    require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,30}", attempt), "invalid attempt ID")
    require(set(config["pins"]) == set(IMAGES) and all(HEX_ID.fullmatch(p) for p in config["pins"].values()),
            "exact binary and script pins required")
    pc, laptop = (ipaddress.IPv4Address(config[name]["ipv4"]) for name in ("pc", "laptop"))
    require(pc != laptop and pc.is_private and laptop.is_private, "two distinct private IPv4 addresses required")
    require(config["pc"]["hostname"].casefold() != config["laptop"]["hostname"].casefold(), "distinct hosts required")
    for name in ("pc", "laptop"):
        first, last = config[name]["udp_ephemeral"]
        require(isinstance(first, int) and isinstance(last, int) and 1024 <= first <= last <= 65535,
                f"{name}: verified Windows UDP range required")
    ssh = pinned_ssh(config["ssh"], pc, laptop)
    evidence = Path(config["evidence"])
    evidence.mkdir()
    deadline = time.monotonic() + 700
    hosts = (Host("pc", config["pc"], str(laptop), attempt, config["pins"], [], evidence, deadline),
             Host("laptop", config["laptop"], str(pc), attempt, config["pins"], ssh, evidence, deadline))
    a, b = hosts
    transfers, messages, invocations, snapshots = [], [], [], []
    success = False

    def start(host, tag, stack, instance=None):
        terminal = Terminal(host.command("chat", tag=tag, observe_socket=instance is None), deadline)
        terminal.tag = tag
        invocations.append((host, tag, terminal, instance is not None))
        stack.callback(terminal.close)
        terminal.connect(instance)
        return terminal

    def ready(sides, live=True):
        until(lambda end: all(t.ready(end, live) for _, t in sides), 65, deadline, "terminal live or monitor not ready")

    def exchange(sides):
        records = []
        for i, (host, terminal) in enumerate(sides):
            host.socket(terminal.tag, deadline)
            def export(end):
                result = terminal.result("locator_export", {}, end)
                if result.get("code") == "locator_unavailable":
                    return None
                require(result["ok"] and result["value"]["state"] == "exported", "locator export")
                return result["value"]["path"]
            path = until(export, 35, deadline, f"{host.name}: locator export deadline")
            dest, receiver = sides[1 - i]
            copied = transfer(host, dest, path, f"locator-{len(transfers)}.json", transfers)
            records.append((receiver, copied))
        for receiver, path in records:
            terminal_value = receiver.call("locator_import", path=path)
            require(terminal_value["state"] == "imported" and terminal_value["retained"], "locator import")
        ready(sides)

    def message(sides, i, slot, label, offline=False):
        sender = sides[i][1]
        receiver = sides[1 - i][1]
        require(sender.ready(deadline, not offline), "sender monitor left watching state")
        if not offline:
            require(receiver.ready(deadline, True), "receiver left live/monitor readiness")
        key = {"context": sender.contexts[slot], "request_id": uuid.uuid4().hex}
        text = f"{attempt} | {label} | LAN ⇄"
        row = {"slot": slot, "sender": i, "receiver": 1 - i, "text": text, "key": key}
        messages.append(row)
        result = sender.call("send", **key, text=text)
        require(result["state"] in PENDING | {"delivered"}, "send state")
        if offline:
            require(result["state"] in PENDING, "send claimed delivery while recipient was stopped")
            pending = sender.call("request_status", **key)
            require(pending["state"] in PENDING, "expected durable pending request")
            row["offline_state"] = pending["state"]
            save(evidence / "offline-request.json", row)
        return row

    def delivered(sides, row):
        sender = sides[row["sender"]][1]
        receiver = sides[row["receiver"]][1]
        def check(end):
            record = sender.call("request_status", end, **row["key"])
            return record["event_id"] if record["state"] == "delivered" else None
        event_id = until(check, 65, deadline, "terminal delivery not confirmed")
        require(HEX_ID.fullmatch(event_id), "delivery event identity")
        row.update(event_id=event_id, receiver_tag=receiver.tag,
                   receiver_instance=receiver.instance, receiver_epoch=receiver.epoch)
        def incoming(_):
            return [event for event in receiver.events if event["event_id"] == event_id]
        events = until(incoming, 40, deadline, "monitor incoming not observed")
        require(len(events) == 1 and events[0]["text"] == row["text"] and events[0]["context"] == row["key"]["context"]
                and events[0]["runtime_instance"] == receiver.instance and events[0]["epoch"] == receiver.epoch,
                "monitor identity, duplicate or content")
        agreement(sides, row["slot"], event_id, row["text"])

    try:
        fixtures = prepare_fixtures(hosts, transfers)
        save(evidence / "fixtures.json", fixtures)

        with ExitStack() as stack:
            left, right = start(a, "pc-owner-0", stack), start(b, "laptop-owner-0", stack)
            sides = [(a, left), (b, right)]
            require(all(t.contexts == fixtures[0]["contexts"] for _, t in sides), "terminal fixture scope")
            exchange(sides)
            for slot in (0, 1):
                require(all(not history(t, t.contexts[slot]) for _, t in sides), "expected empty initial history")
                for i in (0, 1):
                    delivered(sides, message(sides, i, slot, f"context-{slot + 1}-from-{i}"))

            stopped_instance = right.instance
            right.quit()
            b.receipt("laptop-owner-0", descriptor=False)
            pending = message(sides, 0, 1, "queued-while-laptop-offline", offline=True)
            right = start(b, "laptop-owner-1", stack)
            require(right.instance != stopped_instance, "recipient runtime did not restart")
            ready([(b, right)], live=False)
            sides[1] = (b, right)
            exchange(sides)
            delivered(sides, pending)

            stopped_instance = left.instance
            left.quit()
            a.receipt("pc-owner-0", descriptor=False)
            left = start(a, "pc-owner-1", stack)
            require(left.instance != stopped_instance, "sender runtime did not restart")
            sides[0] = (a, left)
            exchange(sides)
            pending["sender_restart"] = {"before": stopped_instance, "after": left.instance, "replies": {}}
            for op in ("request_status", "retry"):
                report = left.call(op, **pending["key"])
                pending["sender_restart"]["replies"][op] = report
                require(report["state"] == "delivered" and report["event_id"] == pending["event_id"], f"{op}: durable event")

            observer = start(b, "laptop-observer", stack, right.instance)
            ready([(b, observer)])
            require(observer.call("disconnect") == {"connected": False}, "observer detach")
            observer.quit()
            b.receipt("laptop-observer", descriptor=True)
            ready(sides)
            delivered(sides, message(sides, 0, 0, "owner-still-live-after-observer-detach"))

            for host, terminal in sides:
                snapshot = {"host": host.name, "tag": terminal.tag, "contexts": []}
                snapshots.append(snapshot)
                for context_id in terminal.contexts:
                    snapshot["contexts"].append({"context": context_id, "rows": history(terminal, context_id)})
            verify_final_history(snapshots, messages)
            left.quit()
            right.quit()

        all_events = [event for _, _, terminal, _ in invocations for event in terminal.events]
        require(sorted(event["event_id"] for event in all_events) == sorted(row["event_id"] for row in messages),
                "duplicate, unexpected or missing event across terminal sessions")
        for host, tag, _, observer in invocations:
            if not (evidence / f"{host.name}-{tag}-job.json").exists():
                host.receipt(tag, descriptor=None if observer else False)
        for host in hosts:
            require(not host.call("descriptors")["descriptor"], "owner descriptor remains")
        success = True
    finally:
        missing = []
        for host, tag, _, _ in invocations:
            if not (evidence / f"{host.name}-{tag}-job.json").exists():
                try:
                    host.receipt(tag, descriptor=None, cleanup=True)
                except Exception as exc:
                    missing.append(f"{host.name}/{tag}: {exc}")
        processes = [{"host": host.name, "tag": tag, "client_pid": term.proc.pid,
                      "wrapper_exit": term.proc.returncode, "quit_exit_and_eof_verified": term.quit_verified,
                      "incoming": list(term.events), "last_readiness": term.last_readiness,
                      "live_readiness": term.live_readiness}
                     for host, tag, term, _ in invocations]
        save(evidence / "pilot.json", {"attempt": attempt, "passed": success, "pins": config["pins"],
                                      "transfers": transfers, "messages": messages, "processes": processes,
                                      "final_history": snapshots, "missing_receipts": missing})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--run-reviewed-pilot", action="store_true")
    args = parser.parse_args()
    require(args.run_reviewed_pilot, "use --run-reviewed-pilot after the plan is accepted")
    run(json.loads(args.config.read_text(encoding="utf-8-sig")))


if __name__ == "__main__":
    main()
