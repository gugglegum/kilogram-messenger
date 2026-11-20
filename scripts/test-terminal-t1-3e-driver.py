"""T1.3e field controller. --preflight is ordinary read-only; --run is the reviewed scenario."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
import json
from pathlib import Path, PureWindowsPath
import re
import runpy
import subprocess
import threading
import time
import uuid


SCRIPTS = Path(__file__).resolve().parent
prior = runpy.run_path(str(SCRIPTS / "test-terminal-t1-2e-driver.py"))
interactive_field = runpy.run_path(str(SCRIPTS / "test-terminal-t1-3e-interactive-field.py"))
Host, MailboxTerminal, require, save, pinned_ssh, history, until = (
    prior[key] for key in ("Host", "MailboxTerminal", "require", "save", "pinned_ssh", "history", "until")
)
HEX = re.compile(r"[a-f0-9]{64}")
ROLLBACK_REPLY_TIMEOUT = 30
HOLD_CLEANUP_RESERVE = 20
PIN = "SHA256:vvR2wQIJmbHD5033l5xVMbrSGyUcucNCy711PYU4oyc"
BASES = (
    r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f",
    r"C:\Users\test\KilogramLab\terminal-t11f",
)
HOSTS = (
    ("A", "DESKTOP-PAUL-RY", "192.168.0.140", 22,
     "b80a5d151daa4d9e8a23d44643e3c46a", "cb82ea10fb0e4911addd60d333a92da9"),
    ("B", "PAUL_HP_OMEN", "192.168.0.152", 19,
     "e70ed7fbd0d94aba88df364117667047", "1a46b2bb8f8e473586d59d2001781b7d"),
)
PINS = {
    "kilogram-cli.exe": "736827d50c3cfb4f34282b6651dccc82f25131c4c86a8594105771a38cda1ac2",
    "kilogram-bootstrap.exe": "dac789937cdb1743db49face9a5df23ddad89c33e649658ae33ad95ce45a9e7a",
    "host.ps1": "13809ae164ef94083e376826bd37f6db08fb5c65f4e76e36316600d8b90463c9",
    "deadline.ps1": "4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5",
    "mailbox.cs": "a26757a0dac1bb440a746db4ca41687e9209a00c24f9f83900fcf8812f2568a6",
    "setup-pipe.cs": "a738983b332563b186a91e65a0880303989d4d2048568c94ef006a94e8a6183c",
}
PENDING = ("queued", "materialized")


class Inconclusive(RuntimeError):
    """A required field precondition was unavailable."""


def parallel(fn, values, *, on_error=None):
    failures = []
    failure_lock = threading.Lock()

    def invoke(value):
        try:
            return fn(value)
        except BaseException as error:
            with failure_lock:
                if not failures:
                    failures.append(error)
            if on_error is not None:
                on_error()
            raise

    try:
        with ThreadPoolExecutor(max_workers=2) as executor:
            return list(executor.map(invoke, values))
    except BaseException:
        if failures:
            raise failures[0]
        raise


def binding(index):
    return {"mode": "auto", "adapter_id": HOSTS[index][4], "network_id": HOSTS[index][5], "prefix": 24}


def network_matches(index, doc):
    _, _, ip, ifindex, adapter, network = HOSTS[index]
    return doc == {"ip": ip, "index": ifindex, "adapter_id": adapter, "udp": [49152, 65535],
                   "binding": [{"network_id": network, "category": 1}]}


def observer_command(source, request):
    content = base64.b64encode(gzip.compress(source, mtime=0)).decode("ascii")
    loader = (
        "$j=[Console]::In.ReadLine();"
        "$m=[IO.MemoryStream]::new([Convert]::FromBase64String([Console]::In.ReadLine()));"
        "$z=[IO.Compression.GZipStream]::new($m,[IO.Compression.CompressionMode]::Decompress);"
        "$s=[IO.StreamReader]::new($z,[Text.Encoding]::UTF8);"
        "& ([ScriptBlock]::Create($s.ReadToEnd())) -Request $j"
    )
    encoded = base64.b64encode(loader.encode("utf-16-le")).decode("ascii")
    require(len(encoded) < 7400, "Windows observer command length")
    stdin = (json.dumps(request, separators=(",", ":"), ensure_ascii=True) + "\n" + content + "\n").encode("ascii")
    return ["-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], stdin


def check_gate(gate, index, op, attempt, audit, sid, phase=None):
    require(gate["host"] == HOSTS[index][1] and gate["side"] == HOSTS[index][0] and gate["attempt"] == attempt
            and gate["op"] == op and gate["audit"] == audit and gate["uid"] == sid and gate["frequency"] > 0,
            "gate session identity or operation differs")
    if phase is not None:
        require(gate["phase"] == phase and gate["state"] == ("released" if op == "R" else "held")
                and gate["tick"] < gate["expires"], "phase hold expired, failed or released unexpectedly")
    else:
        require(gate["state"] == ("rolled_back" if op == "X" else "open"), "unexpected gate state")
    return gate


def sample_endpoint(index, term, doc, job, identity):
    require(doc["runtime_instance"] == term.instance and doc["runtime_pid"] == job["pid"]
            and doc["runtime_started"] == job["started"] and doc["runtime_image"].casefold() == job["image"].casefold()
            and doc["token"] == identity and not doc["token"]["administrator"], "actual runtime or token scope")
    require(doc["discovery"]["selection"] == binding(index), "runtime selected a different LAN binding")
    slots = doc["slots"]
    require([r["slot"] for r in slots] == [0, 1] and [r["context"] for r in slots] == term.contexts
            and all(not r["blocked"] and not r["policy_error"] for r in slots), "two installed usable contexts")
    if not all(r["publication_ready"] for r in slots):
        return None
    endpoint = doc["endpoint"]
    require(len(endpoint["ips"]) == 1 and not endpoint["relays"] and HEX.fullmatch(endpoint["id"]),
            "one direct endpoint")
    address = endpoint["ips"][0]
    ip, port = address.rsplit(":", 1)
    port = int(port)
    expected = [row for row in doc["sockets"] if row["ip"] == ip and row["port"] == port]
    require(ip == HOSTS[index][2] and 49152 <= port <= 65535 and len(expected) == 1,
            "actual owned UDP table differs from direct endpoint")
    return {"address": address, "port": port, "endpoint": endpoint["id"], "pid": doc["runtime_pid"],
            "instance": term.instance, "own": [r["own_locator_revision"] for r in slots]}


def live_pair(samples, endpoints):
    for index, doc in enumerate(samples):
        other = endpoints[1 - index]
        if any(row["phase"] != 3 or not row["active"] or not row["live_admitted"] or not row["lease_expires"]
               or row["selected_lan_peer"] != other["address"]
               or row["peer_locator_revision"] != other["own"][slot] for slot, row in enumerate(doc["slots"])):
            return False
    return True


def pending(value, receiver_rows, row):
    require(value["state"] in PENDING, "request advanced during UDP hold or offline")
    observed = value.get("event_id")
    require(not any(r["text"] == row["text"] or observed and r["event_id"] == observed for r in receiver_rows),
            "recipient already has the held message")
    if observed:
        require(not row.get("pending_event") or row["pending_event"] == observed, "pending event ID changed")
        row["pending_event"] = observed


class Field:
    def __init__(self, config, mode):
        self.attempt = config["attempt"]
        require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,30}", self.attempt), "attempt ID")
        require(config["ssh"]["fingerprint"] == PIN and config["ssh"]["user"] == "test", "reviewed SSH pin required")
        self.source = (SCRIPTS / "test-terminal-t1-3e-observe.ps1").read_bytes()
        require(hashlib.sha256(self.source).hexdigest() == config["observer_sha256"], "observer source SHA-256 mismatch")
        self.approvals = config.get("approvals", {})
        if mode == "run":
            require(set(self.approvals) == {"A", "B"} and all(HEX.fullmatch(v["audit"]) and v["token"]["user"]
                    for v in self.approvals.values()), "both reviewed administrative audits and test tokens required")
        ssh = pinned_ssh(config["ssh"], HOSTS[0][2], HOSTS[1][2])
        self.evidence = SCRIPTS.parent / ".tmp" / "terminal-t1-3e" / f"{self.attempt}-{mode}"
        self.evidence.mkdir(parents=True, exist_ok=False)
        self.started = time.monotonic()
        self.end = self.started + 1130
        self.hosts = [Host(side, {"hostname": machine, "ipv4": ip, "base": BASES[index],
                                 "udp_ephemeral": [49152, 65535]}, HOSTS[1 - index][2], self.attempt, PINS,
                           ssh if index else [], self.evidence, self.end)
                      for index, (side, machine, ip, *_) in enumerate(HOSTS)]
        self.interactive = (interactive_field["Interactive"](self.hosts[1], self.attempt, PINS,
                            self.approvals["B"]["token"]["user"], config["interactive_runner"]) if mode == "run" else None)
        self.processes = []
        self.receipts = set()
        self.messages = []
        self.transfers = []
        self.ports = [set(), set()]
        self.instances = [set(), set()]
        self.prior_endpoint = [None, None]
        self.pair = [None, None]
        self.gates = []

    def observe(self, index, op, end, **params):
        side, machine, *_ = HOSTS[index]
        request = {"op": op, "side": side, "host": machine, "attempt": self.attempt, **params}
        args, stdin = observer_command(self.source, request)
        timeout = ROLLBACK_REPLY_TIMEOUT if op == "gate" and params.get("command") == "X" else 12
        result = subprocess.run(self.hosts[index].ps + args, input=stdin, capture_output=True,
                                timeout=prior["prior"]["remaining"](end, timeout))
        stage = re.search(rb"\bT13E_(?:STATUS2|RUNTIME)_[A-Z_]+\b", result.stderr)
        suffix = f"/{stage.group().decode()}" if stage else ""
        require(result.returncode == 0 and not result.stderr and 0 < len(result.stdout) <= 128000,
                f"{side}/{op}{suffix}: observer failed ({result.returncode})")
        return json.loads(result.stdout.decode("utf-8-sig"))

    def preflight(self, index):
        end = min(self.end, time.monotonic() + 30)
        token = self.observe(index, "identity", end)
        network = self.observe(index, "network", end)
        ports = self.observe(index, "ports", end)["sockets"]
        save(self.evidence / f"{HOSTS[index][0]}-preflight.json", {"token": token, "network": network,
             "multicast_socket_users": [s for s in ports if s["port"] == 50372]})
        require(network_matches(index, network) and not token["administrator"]
                and not any(s["port"] == 50372 for s in ports), "preflight binding, token or discovery port changed")
        return token

    def gate(self, index, op, end, phase=None):
        side = HOSTS[index][0]
        before = time.monotonic()
        row = self.observe(index, "gate", end, command=op)
        self.gates.append({"sent": before - self.started, "received": time.monotonic() - self.started, "reply": row})
        approved = self.approvals[side]
        return check_gate(row, index, op, self.attempt, approved["audit"], approved["token"]["user"], phase)

    def start(self, index, tag, mode, end=None):
        host = self.hosts[index]
        term = MailboxTerminal(host, tag, mode, end or self.phase_end,
                               interactive=self.interactive if index else None,
                               **({"observe_socket": True} if mode == "chat" else {}))
        self.processes.append((index, term))
        if mode == "chat":
            require(term.line(term.deadline) == {"v": 1, "type": "ready"}, "Chat JSONL readiness")
            require(term.call("status") == {"connected": False}, "prestarted Chat is connected")
            proof = self.observe(index, "preconnect", term.deadline, tag=tag)
            require(proof["disconnected"] and not proof["descriptor"] and not proof["sockets"],
                    "prestarted Chat created a runtime or socket")
            save(self.evidence / f"{host.name}-{tag}-prestart.json", proof)
            return term
        require(term.line(self.phase_end)["type"] == "ready", "Setup ready")
        return term

    def receipt(self, index, term, cleanup=False):
        if term.tag == "setup" and (self.evidence / f"{HOSTS[index][0]}-setup-job.json").exists():
            # onboard() already demanded the full Job receipt and protected EOF.
            doc = json.loads((self.evidence / f"{HOSTS[index][0]}-setup-job.json").read_bytes())
            require(doc["filtered_setup_eof"] and doc["mailbox_eof"] and doc["mailbox_quit"]
                    and doc["membership"] and not doc["after"]
                    and not doc["descriptor"] and not doc["error"] and doc["child_exit"] == 0
                    and not doc["forced"] and term.quit_verified, "Setup receipt failed")
        else:
            self.hosts[index].receipt(term.tag, descriptor=False, cleanup=cleanup)
            doc = json.loads((self.evidence / f"{HOSTS[index][0]}-{term.tag}-job.json").read_bytes())
            require(doc["mailbox_eof"] and (cleanup or doc["mailbox_quit"]), "mailbox EOF or quit is unconfirmed")
        self.receipts.add((index, term.tag))

    def select_route(self, host, term):
        index = int(host.name == "B")
        chosen = term.call("route_lan_auto", local_ipv4=host.config["ip"], peer_ipv4=host.config["peer"])
        expected = {"mode": "direct_lan", "local_ipv4": host.config["ip"], "peer_ipv4": host.config["peer"],
                    "profile_state": "not_prepared", "lan_discovery": binding(index)}
        require(all(chosen["connection"].get(key) == value for key, value in expected.items()),
                "saved opted-in Auto LAN selection")
        self.roles[index] = {"proposer": 0, "cosigner": 1}[chosen["consent"]["role"]]
        return chosen

    def sample(self, index, term, end):
        host = self.hosts[index]
        host.socket(term.tag, end)
        job = json.loads((self.evidence / f"{host.name}-{term.tag}-socket.json").read_bytes())
        params = {"sha256": PINS["kilogram-cli.exe"], "runtime_pid": job["pid"],
                  "runtime_started": job["started"], "runtime_image": job["image"]}
        if index:
            params["owner_token"] = self.owner_tokens[index]
            params["owner_session"] = self.interactive.ready["session"]
        doc = self.observe(index, "status", end, **params)
        if not doc["ready"]:
            return None
        point = sample_endpoint(index, term, doc, job, self.owner_tokens[index])
        require([r["role"] for r in doc["slots"]] == [self.roles[index]] * 2, "consent role changed")
        return (doc, point) if point else None

    def stop(self, index, term):
        term.quit()
        self.receipt(index, term)

    def port_gone(self, index, port, label, end):
        sockets = self.observe(index, "ports", end)["sockets"]
        relevant = [s for s in sockets if s["port"] == port]
        save(self.evidence / f"{HOSTS[index][0]}-{label}-port.json", {"port": port, "matches": relevant})
        if relevant:
            raise Inconclusive(f"{HOSTS[index][0]}: old UDP port still occupied")

    def owner_ready(self, index, phase, end):
        require(not self.rollback_started, "phase preparation cancelled")
        term = self.pair[index]
        term.deadline = end
        bound = term.call("connect", end, mode="start")
        require(bound["mode"] == "owner" and bound["contexts"] == self.contexts
                and HEX.fullmatch(bound["runtime_instance"]) and bound["runtime_instance"] not in self.instances[index],
                "fresh runtime and preserved contexts required")
        term.contexts, term.instance, term.epoch = bound["contexts"], bound["runtime_instance"], None
        self.instances[index].add(term.instance)
        doc, point = until(lambda limit: self.sample(index, term, limit), 15, end, "owned LAN socket not ready")
        require(not any(r["live_admitted"] for r in doc["slots"]), "fresh owner admitted while gate held")
        until(lambda limit: self.monitor(index, limit, watching=phase != "A", warm=phase != "A", held=True),
              30, end, "receiver monitor did not anchor")
        save(self.evidence / f"{HOSTS[index][0]}-{term.tag}-ready.json",
             {"endpoint": point, "runtime": doc, "monitor": term.last_readiness})
        if point["port"] in self.ports[index]:
            # A retry would create a new disconnected Chat after Hold began.
            raise Inconclusive(f"{HOSTS[index][0]}: OS reused a previously observed UDP port")
        term.deadline = self.phase_end
        return term, point

    def monitor(self, index, end, *, watching, warm=False, held=False):
        require(not self.rollback_started, "phase preparation cancelled")
        term = self.pair[index]
        status, monitor = term.status(end)
        require(status["mode"] == "owner" and status["runtime_instance"] == term.instance
                and status["contexts"] == self.contexts, "monitor runtime or contexts changed")
        if held:
            require(not status["live_admitted"], "owner admitted before Release")
        epoch = monitor["epoch"]
        if epoch is None:
            require(term.epoch is None, "monitor epoch disappeared")
            return None
        require(type(epoch) is int and epoch > 0 and (term.epoch is None or term.epoch == epoch),
                "monitor epoch changed")
        contexts = monitor["contexts"]
        require([(row["slot"], row["context"]) for row in contexts] == list(enumerate(self.contexts)),
                "monitor context binding changed")
        states = [row["state"] for row in contexts]
        require(all(state in ("anchoring", "waiting_history", "retrying", "watching") for state in states),
                "monitor history gap, damage or unsupported history")
        require(not warm or "waiting_history" not in states, "warm history unavailable")
        term.epoch = epoch
        term.last_readiness = {"runtime_instance": term.instance, "live_admitted": status["live_admitted"],
                               "monitor": monitor}
        return term.last_readiness if not watching or states == ["watching", "watching"] else None

    def monitors(self, phase, end, *, watching, held=False):
        def observe(limit):
            rows = parallel(lambda i: self.monitor(i, limit, watching=watching, warm=phase in ("B", "C"), held=held),
                            (0, 1), on_error=self.rollback)
            return rows if all(rows) else None

        return until(observe, 30, end, "both receiving monitors must be watching")

    def rollback(self):
        # A worker invokes this before ThreadPoolExecutor waits for its sibling.
        # Repeated callers observe one rollback attempt for the current field.
        with self.rollback_lock:
            if self.rollback_started:
                return
            self.rollback_started = True

        def close_gate(index):
            try:
                self.gate(index, "X", min(self.started + 1168, time.monotonic() + ROLLBACK_REPLY_TIMEOUT))
            except Exception as failure:
                self.cleanup_errors.append({"host": index, "rollback": str(failure)})

        parallel(close_gate, (0, 1))

    def admit(self, phase, end):
        self.gate(0, "S", end, phase)
        released_at = time.monotonic()
        self.gate(0, "R", end, phase)

        def check(limit):
            reports = parallel(lambda i: self.sample(i, self.pair[i], limit), (0, 1))
            if any(report is None for report in reports):
                return None
            docs, endpoints = map(list, zip(*reports))
            if not live_pair(docs, endpoints):
                return None
            return docs, endpoints

        docs, endpoints = until(check, 90, min(end, released_at + 90), f"{phase}: four current live leases")
        measured = time.monotonic() - released_at
        require(measured <= 90, "four live leases exceeded 90 s")
        save(self.evidence / f"{phase}-admitted.json", {"seconds": measured, "hosts": docs, "endpoints": endpoints})
        return endpoints

    def export(self, index, phase, point, end):
        host, term = self.hosts[index], self.pair[index]
        path = PureWindowsPath(term.call("locator_export", end)["path"].removeprefix("\\\\?\\"))
        require(path.parent == host.setup / "connection", "diagnostic export must stay on source host")
        reply = host.call("get", name=path.name, timeout=prior["prior"]["remaining"](end, 12))
        data = base64.b64decode(reply["data"], validate=True)
        require(hashlib.sha256(data).hexdigest() == reply["sha256"], "source public locator changed")
        local = self.evidence / f"{host.name}-{phase}-locator.json"
        with local.open("xb") as file:
            file.write(data)
        parsed = prior["signed_socket"](local, term.contexts)
        require(all(f'{s["ip"]}:{s["port"]}' == point["address"] and s["endpoint"] == point["endpoint"]
                    and s["revision"] == point["own"][i] for i, s in enumerate(parsed)),
                "signed LAN locator differs from actual and admitted endpoint")

    def send(self, sender, slot, label, end):
        term = self.pair[sender]
        row = {"sender": sender, "slot": slot, "text": f"{self.attempt} {label}",
               "key": {"context": term.contexts[slot], "request_id": uuid.uuid4().hex}}
        save(self.evidence / f"intent-{label}.json", row)
        reply = term.call("send", end, **row["key"], text=row["text"])
        require(reply["state"] in (*PENDING, "delivered"), "unexpected send result")
        row["pending_event"] = reply.get("event_id")
        self.messages.append(row)
        return row

    def check_pending(self, row, end, label, receiver_present=True):
        sender = self.pair[row["sender"]]
        receiver = self.pair[1 - row["sender"]]
        report = sender.call("request_status", end, **row["key"])
        rows = history(receiver, row["key"]["context"], end) if receiver_present else []
        pending(report, rows, row)
        save(self.evidence / f"pending-{label}.json", {"request": report, "receiver_history": rows})

    def outage_pending(self, by_sender, number, end):
        # Each task exclusively owns one mailbox sequence/cursor during the sample.
        def probe(index):
            terminal = self.pair[index]
            request = terminal.call("request_status", end, **by_sender[index]["key"])
            return request, history(terminal, by_sender[1 - index]["key"]["context"], end)

        samples = parallel(probe, (0, 1))
        for index, row in enumerate(by_sender):
            report, rows = samples[index][0], samples[1 - index][1]
            pending(report, rows, row)
            receiver = self.pair[1 - index]
            require(not any(event["text"] == row["text"] for event in receiver.events),
                    "incoming event observed during hold")
            save(self.evidence / f"pending-hold-{index}-{number}.json", {"request": report, "receiver_history": rows})

    def deliver(self, row, end, retry=False):
        sender = self.pair[row["sender"]]
        receiver = self.pair[1 - row["sender"]]

        def check(limit):
            value = sender.call("request_status", limit, **row["key"])
            return value if value["state"] == "delivered" else None

        result = until(check, 35, end, "message did not reach delivered")
        row["event_id"] = result["event_id"]
        require(HEX.fullmatch(row["event_id"]) and (not row.get("pending_event") or
                row["pending_event"] == row["event_id"]), "request mapped to a different event")
        if retry:
            repeated = sender.call("retry", end, **row["key"])
            require(repeated["state"] == "delivered" and repeated["event_id"] == row["event_id"], "retry changed event")
            row["retry"] = repeated
        row.update(receiver_instance=receiver.instance, receiver_epoch=receiver.epoch)
        row["notification"] = prior["observe_incoming"](receiver, row, end)
        require(row["notification"]["status"] == "confirmed", "missing or duplicate receiver notification")

    def collect_histories(self, end):
        return parallel(lambda index: {"host": self.hosts[index].name, "contexts": [
            {"context": ctx, "rows": history(self.pair[index], ctx, end)} for ctx in self.contexts]}, (0, 1))

    def histories(self, phase, end):
        snapshots = self.collect_histories(end)
        prior["verify_final_history"](snapshots, self.messages)
        save(self.evidence / f"{phase}-history.json", snapshots)

    def phase(self, phase):
        budget = dict(A=145, B=165, C=170, D=170)[phase]
        end = self.phase_end = min(self.end - 30, time.monotonic() + budget)
        for host in self.hosts:
            host.deadline = end
        for term in self.pair:
            if term:
                term.deadline = end
        previous = self.prior_endpoint.copy()
        restart = dict(A=(0, 1), B=(1,), C=(0, 1), D=())[phase]
        pending_rows = []
        if phase in ("B", "C"):
            def close_previous(index):
                self.stop(index, self.pair[index])
                self.port_gone(index, previous[index]["port"], f"{phase}-closed", end)
            parallel(close_previous, restart)
        if phase == "B":
            pending_rows.append(self.send(0, 1, "offline", end))
            self.check_pending(pending_rows[0], end, "offline-before-start", receiver_present=False)
        if phase == "D":
            self.network_proof("D-before", end)
        if restart:
            terminals = parallel(lambda i: self.start(i, f"owner-{phase.lower()}-1", "chat", end),
                                 restart, on_error=self.rollback)
            for index, term in zip(restart, terminals):
                self.pair[index] = term
        started = time.monotonic()
        hold = self.gate(0, phase, end, phase)
        reserve = 2 if phase == "D" else HOLD_CLEANUP_RESERVE
        held_until = min(end, started + (hold["expires"] - hold["tick"]) / hold["frequency"] - reserve)
        if restart:
            prepared = parallel(lambda i: self.owner_ready(i, phase, held_until), restart, on_error=self.rollback)
            for index, (term, point) in zip(restart, prepared):
                self.pair[index] = term
                self.ports[index].add(point["port"])
                self.prepared[index] = point
        if phase == "B":
            self.check_pending(pending_rows[0], held_until, "offline-watching")
        if phase == "D":
            pending_rows = parallel(lambda i: self.send(i, i, f"hold-{i}", held_until), (0, 1))
            started_sample = time.monotonic()
            self.outage_pending(pending_rows, 1, held_until)
            self.network_proof("D-held", held_until)
            wait = max(5, 20 - (time.monotonic() - started))
            require(time.monotonic() + wait < held_until, "hold evidence exceeds the bounded outage window")
            time.sleep(wait)
            require(time.monotonic() - started_sample >= 5, "hold samples too close")
            self.outage_pending(pending_rows, 2, held_until)
            require(time.monotonic() - started <= 35, "outage exceeded 35 seconds before release")
        pre_release = self.monitors(phase, held_until, watching=phase != "A", held=phase in ("A", "B", "C"))
        save(self.evidence / f"{phase}-pre-release-monitor.json", pre_release)
        require(time.monotonic() < held_until, "hold deadline passed during phase preparation")
        points = self.admit(phase, end)
        if phase == "A":
            monitors = self.monitors(phase, end, watching=True)
            save(self.evidence / "A-anchored-monitor.json", monitors)
            require(all(not rows for rows in parallel(lambda i: [r for ctx in self.contexts for r in
                    history(self.pair[i], ctx, end)], (0, 1))), "initial histories are not empty")
        if phase == "D":
            released = self.gates[-1]["reply"]
            require(20 <= (released["tick"] - hold["tick"]) / hold["frequency"] <= 35,
                    "confirmed UDP outage outside 20–35 seconds")
            self.network_proof("D-after", end)
        for i, point in enumerate(points):
            old = previous[i]
            if i in restart:
                require(point["instance"] == self.prepared[i]["instance"] and point["port"] == self.prepared[i]["port"],
                        "new socket changed after release")
                if old:
                    require(point["endpoint"] != old["endpoint"] and all(x > y for x, y in zip(point["own"], old["own"])),
                            "new owner endpoint ID or revisions did not advance")
            elif old and phase != "D":
                require(point["instance"] == old["instance"] and point["address"] == old["address"]
                        and point["endpoint"] == old["endpoint"], "unexpected restart of the other host")
        parallel(lambda i: self.export(i, phase, points[i], end), (0, 1))
        self.prior_endpoint = points
        for row in pending_rows:
            self.deliver(row, end, retry=True)
        if phase != "B":
            self.monitors(phase, end, watching=True)
            for slot in (0, 1):
                self.deliver(self.send(1 - slot if phase == "C" else slot, slot, f"{phase.lower()}-{slot}", end), end)
        self.histories(phase, end)
        observed = parallel(lambda i: self.sample(i, self.pair[i], end), (0, 1))
        require(all(report and report[1] == points[i] for i, report in enumerate(observed))
                and live_pair([report[0] for report in observed], points), "post-delivery route or lease changed")

    def network_proof(self, label, end):
        docs = parallel(lambda i: self.observe(i, "network", end), (0, 1))
        save(self.evidence / f"{label}-network.json", docs)
        require(all(network_matches(i, doc) for i, doc in enumerate(docs)), "pinned network changed")

    def run(self):
        self.phase_end = min(self.end, time.monotonic() + 450)
        self.roles = [None, None]
        self.prepared = {}
        cleanup = self.cleanup_errors = []
        self.rollback_lock = threading.Lock()
        self.rollback_started = False
        phases = []
        failure_info = None
        for host in self.hosts:
            host.deadline = self.phase_end
        try:
            self.tokens = parallel(self.preflight, (0, 1))
            require(all(self.tokens[i] == self.approvals[HOSTS[i][0]]["token"] for i in (0, 1)),
                    "actual launcher identities differ from reviewed identities")
            self.owner_tokens = self.tokens.copy()
            if self.interactive:
                ready = self.interactive.read_ready(self.phase_end)
                self.owner_tokens[1] = ready["token"]
                save(self.evidence / "B-interactive-ready.json", ready)
            states = parallel(lambda i: self.gate(i, "S", self.phase_end), (0, 1))
            require(min(doc["seconds_left"] for doc in states) >= 1180, "insufficient watchdog budget")
            if self.interactive:
                received = next(gate["received"] for gate in self.gates if gate["reply"] is states[1])
                self.interactive.arm(states[1], received=self.started + received,
                                     started=self.started, end=self.phase_end)
            self.contexts = prior["onboard"](self.hosts, lambda host: self.start(int(host.name == "B"), "setup", "setup"),
                                             self.evidence, self.transfers, self.select_route)
            for index, term in self.processes:
                self.receipt(index, term)
            for phase in "ABCD":
                self.phase(phase)
                phases.append(phase)
            parallel(lambda i: self.stop(i, self.pair[i]), (0, 1))
            events = [event for _, term in self.processes for event in term.events]
            require(len(events) == len(self.messages) and
                    all(prior["incoming_result"](events, row)["status"] == "confirmed" for row in self.messages),
                    "unexpected or replayed notification after shutdown")
        except BaseException as failure:
            failure_info = {"kind": type(failure).__name__, "message": str(failure)}
            raise
        finally:
            # Cancel the network hold before potentially slow/failed chat shutdown.
            # Owner preparation also calls this directly on its first failure.
            self.rollback()
            def close_owned(index):
                for side, term in self.processes:
                    if side != index or (side, term.tag) in self.receipts:
                        continue
                    try:
                        term.close()
                    except Exception as failure:
                        cleanup.append({"host": side, "tag": term.tag, "error": str(failure)})
                    try:
                        self.receipt(side, term, cleanup=True)
                    except Exception as failure:
                        cleanup.append({"host": side, "tag": term.tag, "receipt_error": str(failure)})
            parallel(close_owned, (0, 1))
            if self.interactive and self.interactive.ready:
                try:
                    row = self.interactive.finish(min(self.started + 1160, time.monotonic() + 18))
                    save(self.evidence / "B-interactive-result.json", row)
                    require(row["state"] == "closed" and row["empty_job"] and not row["forced"] and row["error"] is None,
                            "B ordinary runner did not close cleanly")
                except Exception as failure:
                    cleanup.append({"host": 1, "runner": str(failure)})
            save(self.evidence / "result.json", {"phases": phases, "failure": failure_info,
                 "cleanup": cleanup, "messages": self.messages,
                 "gates": self.gates, "transfers": self.transfers, "elapsed": time.monotonic() - self.started})
        require(phases == list("ABCD") and not cleanup, "field run or cleanup incomplete")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--preflight", action="store_true")
    mode.add_argument("--run-reviewed-field", action="store_true")
    args = parser.parse_args()
    config = json.loads(args.config.read_text(encoding="utf-8-sig"))
    field = Field(config, "preflight" if args.preflight else "run")
    if args.preflight:
        parallel(field.preflight, (0, 1))
    else:
        field.run()


if __name__ == "__main__":
    main()
