"""Run the reviewed T1.2e field pilot over the existing T1.1f host launcher."""
import argparse
import base64
from contextlib import ExitStack
import hashlib
import ipaddress
import json
from pathlib import Path
import runpy
import time
import uuid

prior = runpy.run_path(str(Path(__file__).with_name("test-terminal-t1-1f-driver.py")))
MailboxTerminal = runpy.run_path(str(Path(__file__).with_name("test-terminal-t1-2e-mailbox.py")))["MailboxTerminal"]
Host, Terminal, require, save, pinned_ssh, transfer, history, until, verify_final_history = (
    prior[k] for k in ("Host", "Terminal", "require", "save", "pinned_ssh", "transfer", "history", "until",
                       "verify_final_history")
)


class SetupTerminal(Terminal):
    def read(self):
        try:
            while raw := self.proc.stdout.readline(16385):
                require(len(raw) <= 16384 and raw.endswith(b"\n"), "Setup response bound")
                require(not any(word in raw for word in (b"recovery_words", b"recovery_scope", b"host-recovery")),
                        "Unexpected private Setup output")
                row = json.loads(raw)
                require(row["v"] == 1 and row["type"] in {"ready", "result"}, "Setup response type")
                self.rows.put(row)
        except Exception:
            self.rows.put(RuntimeError("Protected Setup output rejected"))
        finally:
            self.rows.put(None)

    def read_errors(self):
        while self.proc.stderr.read(4096):
            self.errors[:] = b"!"

    def quit(self):
        require(self.call("quit") == {"closed": True}, "Setup quit")
        require(self.close() and self.proc.returncode == 0 and not self.errors, "Setup shutdown/streams")
        require(self.rows.get(timeout=1) is None, "Setup EOF")
        self.quit_verified = True


def signed_socket(path, context_ids):
    """Read the public locator array shape for evidence. Runtime verifies signatures."""
    bundle = json.loads(path.read_bytes())

    def fields(raw):
        cursor = 0

        def read(n):
            nonlocal cursor
            require(0 < n <= 8192 and cursor + n <= len(raw), "locator byte bound")
            result = raw[cursor:cursor + n]
            cursor += n
            return result

        def value(depth=0):
            require(depth < 6, "locator depth")
            head = read(1)[0]
            kind, number = divmod(head, 32)
            if head == 246:
                return None
            if number >= 24:
                require(number <= 27, "finite locator field")
                number = int.from_bytes(read(1 << (number - 24)), "big")
            if kind == 0:
                return number
            if kind == 2:
                return read(number) if number else b""
            require(kind == 4 and number <= 16, "locator array shape")
            return [value(depth + 1) for _ in range(number)]

        parsed = value()
        require(cursor == len(raw) and len(parsed) == 2 and len(parsed[1]) == 64, "locator wire")
        return parsed[0]

    result = []
    require(len(bundle["locators"]) == 2 and bundle["contexts"] == context_ids, "locator bundle context scope")
    for context, blob in zip(context_ids, bundle["locators"]):
        row = fields(bytes.fromhex(blob))
        require(len(row) == 10 and row[0] == 2 and row[1].hex() == context and row[6] in (None, []),
                "signed LAN locator profile")
        require(len(row[5]) == 1 and row[5][0][0] == 4, "one signed IPv4 UDP socket")
        _, ipv4, port = row[5][0]
        result.append({"context": context, "ip": str(ipaddress.IPv4Address(ipv4)), "port": port,
                       "endpoint": row[4].hex(), "revision": row[7]})
    require(len({(row["ip"], row["port"], row["endpoint"]) for row in result}) == 1, "locator pair differs")
    return result


def onboard(hosts, start_setup, evidence, transfers, select_route=None):
    for host in hosts:
        save(evidence / f"{host.name}-init.json", host.call("init"))
    a, b = [start_setup(host) for host in hosts]
    a.call("begin")
    a.call("create")
    ai = a.call("save_test_recovery")
    invited = a.call("invite_export")
    invite = transfer(hosts[0], hosts[1], invited["path"], "invite.kilogram", transfers)
    preview = b.call("invite_inspect", path=invite)
    require(preview["sender"] == {k: ai[k] for k in ("account_id", "device_id")}
            and preview["contexts"] == ai["contexts"]
            and preview["invitation_id"] == invited["invitation_id"], "verified inviter pins")
    b.call("invite_join", preview_id=preview["preview_id"], confirmed=True)
    b.call("create")
    bi = b.call("save_test_recovery")
    reply = transfer(hosts[1], hosts[0], b.call("reply_export")["path"], "reply.kilogram", transfers)
    preview = a.call("reply_inspect", path=reply)
    require(preview["sender"] == {k: bi[k] for k in ("account_id", "device_id")}
            and preview["recipient"] == {k: ai[k] for k in ("account_id", "device_id")}
            and preview["invitation_id"] == invited["invitation_id"]
            and preview["contexts"] == ai["contexts"] == bi["contexts"], "verified reply pins")
    a.call("reply_accept", preview_id=preview["preview_id"], confirmed=True)
    participants = [a.call("status"), b.call("status")]
    roles = [r["consent"]["role"] for r in participants]
    require(sorted(roles) == ["cosigner", "proposer"], "canonical roles")
    require((roles[0] == "proposer") == (ai["device_id"] < bi["device_id"]), "device role order")
    i = roles.index("proposer")
    proposer, cosigner = (a, b) if i == 0 else (b, a)
    proposer.call("consent_propose")
    exported = proposer.call("consent_export")
    path = transfer(hosts[i], hosts[1 - i], exported["path"], "proposal.kilogram", transfers)
    proposal = cosigner.call("consent_inspect", kind="proposal", path=path)
    require(proposal["artifact_digest"] == exported["artifact_digest"], "inspected proposal digest")
    cosigner.call("consent_accept", kind="proposal", preview_id=proposal["preview_id"], confirmed=True)
    cosigner.call("consent_cosign")
    exported = cosigner.call("consent_export")
    path = transfer(hosts[1 - i], hosts[i], exported["path"], "final.kilogram", transfers)
    final = proposer.call("consent_inspect", kind="final", path=path)
    require(final["artifact_digest"] == exported["artifact_digest"], "inspected final digest")
    proposer.call("consent_accept", kind="final", preview_id=final["preview_id"], confirmed=True)
    for index, term in enumerate((a, b)):
        installed = term.call("consent_import")["consent"]
        require(installed["local_installation"] == "confirmed_current_session" and
                [slot["importing"] for slot in installed["slots"]] == ["accepted", "accepted"],
                "two local installs")
        host = hosts[index]
        address, peer = host.config["ip"], host.config["peer"]
        if select_route:
            route = select_route(host, term)
        else:
            route = term.call("route_lan", local_ipv4=address, peer_ipv4=peer)
            require(route["connection"] == {"mode": "direct_lan", "local_ipv4": address,
                                            "peer_ipv4": peer, "profile_state": "not_prepared"}, "saved LAN route")
        participants[index] = {"host": host.config["hostname"], "setup": route}
        term.quit()
        host.receipt("setup", descriptor=False)
        job = json.loads((evidence / f"{host.name}-setup-job.json").read_bytes())
        require(job["filtered_setup_eof"], "host did not drain both protected streams")
    save(evidence / "setup-complete.json", {"participants": participants, "transfers": transfers})
    return ai["contexts"]


def incoming_result(events, row):
    received = [notice for notice in events if notice["event_id"] == row["event_id"]]
    if not received:
        status = "not_observed"
    elif len(received) > 1:
        status = "duplicate"
    else:
        notice = received[0]
        correct = (notice["context"] == row["key"]["context"] and notice["text"] == row["text"]
                   and notice["runtime_instance"] == row["receiver_instance"]
                   and notice["epoch"] == row["receiver_epoch"])
        status = "confirmed" if correct else "binding_mismatch"
    return {"status": status, "events": received}


def observe_incoming(receiver, row, deadline):
    end = min(deadline, time.monotonic() + 35)
    while True:
        result = incoming_result(receiver.events, row)
        left = end - time.monotonic()
        if result["status"] != "not_observed" or left <= 0:
            return result
        if isinstance(receiver, MailboxTerminal) and left > 1:
            receiver.collect(end)
        time.sleep(min(0.25, left))


def notifications_confirmed(messages):
    return all(row["notification"]["status"] == "confirmed" for row in messages)


def run(config):
    started = time.monotonic()
    end = started + 760
    deadline = started + 450
    require("mailbox.cs" in config["pins"] and "mailbox-fake.ps1" not in config["pins"],
            "reviewed field mailbox configuration required")
    evidence = Path(config["evidence"])
    evidence.mkdir(parents=True, exist_ok=False)
    pc, laptop = (config[role]["ipv4"] for role in ("pc", "laptop"))
    remote = pinned_ssh(config["ssh"], pc, laptop)
    hosts = [Host("pc", config["pc"], laptop, config["attempt"], config["pins"], [], evidence, deadline),
             Host("laptop", config["laptop"], pc, config["attempt"], config["pins"], remote, evidence, deadline)]
    owners, processes, messages, transfers, sockets, missing = [], [], [], [], [], []
    progress = {"phase_a": False, "phase_b_delivery": False, "phase_b": False}

    def start(host, tag, interface, stack):
        term = MailboxTerminal(host, tag, interface, deadline,
                               **({"observe_socket": True} if interface == "chat" else {}))
        processes.append((host, term))
        stack.callback(term.close)
        if interface == "chat":
            term.connect()
            owners.append(term)
        else:
            require(term.line(deadline)["type"] == "ready", "Setup ready")
        return term

    def exchange(pair, label):
        destinations = []
        for i, (host, term) in enumerate(zip(hosts, pair)):
            host.socket(term.tag, deadline)
            socket = json.loads((evidence / f"{host.name}-{term.tag}-socket.json").read_bytes())
            def export(limit):
                reply = term.result("locator_export", {}, limit)
                if reply.get("code") == "locator_unavailable":
                    return None
                require(reply["ok"] and reply["value"]["state"] == "exported", "locator export")
                return reply["value"]["path"]
            path = until(export, 30, deadline, "locator export deadline")
            received = transfer(host, hosts[1 - i], path, f"{label}-locator-{i}.json", transfers)
            # Read the public source using the same already pinned get operation as transfer.
            raw = host.call("get", name=Path(path).name)
            body = base64.b64decode(raw["data"], validate=True)
            require(hashlib.sha256(body).hexdigest() == raw["sha256"] == transfers[-1]["sha256"], "public export changed")
            local = evidence / f"{label}-locator-{i}.json"
            with local.open("xb") as out:
                out.write(body)
            locators = signed_socket(local, term.contexts)
            require(all(r["ip"] == socket["ip"] and r["port"] == socket["port"] for r in locators),
                    "signed locator differs from observed bound socket")
            sockets.append({"host": host.name, "tag": term.tag, "runtime_instance": term.instance,
                            "socket": socket, "locators": locators})
            destinations.append(received)
        for i, term in enumerate(pair):
            imported = term.call("locator_import", path=destinations[1 - i])
            require(imported["state"] == "imported" and imported["retained"], "two locator imports")
        until(lambda limit: all(term.ready(limit, True) for term in pair), 65, deadline, "LAN live admission")

    def snapshots(pair):
        return [{"host": host.name, "contexts": [{"context": ctx, "rows": history(term, ctx)}
                 for ctx in term.contexts]} for host, term in zip(hosts, pair)]

    def message(pair, side, slot, label, offline=False):
        sender = pair[side]
        key = {"context": sender.contexts[slot], "request_id": uuid.uuid4().hex}
        row = {"sender": side, "slot": slot, "key": key, "text": config["attempt"] + " " + label}
        if offline:
            save(evidence / "offline-intent.json", row)
        result = sender.call("send", **key, text=row["text"])
        require(result["state"] in {"queued", "materialized", "delivered"}, "send state")
        if offline:
            pending = sender.call("request_status", **key)
            require(pending["state"] in {"queued", "materialized"}, "offline request was not pending")
            save(evidence / "offline-pending.json", pending)
        messages.append(row)
        return row

    def delivered(pair, row, *, early=False):
        sender, receiver = pair[row["sender"]], pair[1 - row["sender"]]
        def check(limit):
            value = sender.call("request_status", limit, **row["key"])
            return value["event_id"] if value["state"] == "delivered" else None
        event = until(check, 65, deadline, "delivery receipt")
        row.update(event_id=event, receiver_tag=receiver.tag, receiver_instance=receiver.instance,
                   receiver_epoch=receiver.epoch, delivered_before_fresh_exchange=early)

    try:
        with ExitStack() as stack:
            context_ids = onboard(hosts, lambda host: start(host, "setup", "setup", stack), evidence, transfers)

            pair = [start(host, "owner-a", "chat", stack) for host in hosts]
            require(all(t.contexts == context_ids for t in pair), "owner context binding")
            before = [t.status(deadline)[0] for t in pair]
            require(not any(v["live_admitted"] for v in before), "initial live status before exchange")
            exchange(pair, "a")
            require(all(not history(t, context) for t in pair for context in t.contexts), "fresh history")
            for slot in (0, 1):
                row = message(pair, slot, slot, "initial-" + str(slot + 1))
                delivered(pair, row)
                row["notification"] = observe_incoming(pair[1 - slot], row, deadline)
                require(notifications_confirmed([row]), "missing, duplicate or mismatched phase A notification")
            initial = snapshots(pair)
            verify_final_history(initial, messages)
            save(evidence / "phase-a.json", {"pre_exchange": before, "live": [t.live_readiness for t in pair],
                 "messages": messages, "history": initial, "sockets": sockets, "transfers": transfers})
            progress["phase_a"] = True
            print("T12E_PHASE_A_PASS", flush=True)

            deadline = min(time.monotonic() + 250, end - 30)
            for host in hosts:
                host.deadline = deadline
            for term in pair:
                term.deadline = deadline
            old = pair[1].instance
            pair[1].quit()
            hosts[1].receipt("owner-a", descriptor=False)
            offline = message(pair, 0, 1, "offline", offline=True)
            pair[1] = start(hosts[1], "owner-b", "chat", stack)
            require(pair[1].instance != old and pair[1].contexts == pair[0].contexts, "receiver restart changed pins")
            until(lambda limit: pair[1].ready(limit, False), 55, deadline, "receiver watching before exchange")
            early = pair[0].call("request_status", **offline["key"])["state"] == "delivered"
            exchange(pair, "b")
            first, second = [proof for proof in sockets if proof["host"] == "laptop"]
            require(all(after["revision"] > before["revision"] for before, after in zip(first["locators"], second["locators"])),
                    "receiver locator revisions did not advance")
            delivered(pair, offline, early=early)
            retry = pair[0].call("retry", **offline["key"])
            require(retry["state"] == "delivered" and retry["event_id"] == offline["event_id"], "duplicate request changed result")
            final = snapshots(pair)
            verify_final_history(final, messages)
            save(evidence / "phase-b-delivery.json", {"offline": offline, "retry": retry, "history": final,
                 "sockets": sockets, "live": [t.live_readiness for t in pair], "old_receiver_instance": old})
            progress["phase_b_delivery"] = True
            offline["notification"] = observe_incoming(pair[1], offline, deadline - 25)
            for host, term in zip(hosts, pair):
                term.quit()
                host.receipt(term.tag, descriptor=False)
            events = [notice for term in owners for notice in term.events]
            for row in messages:
                row["notification"] = incoming_result(events, row)
            passed = notifications_confirmed(messages)
            save(evidence / "phase-b.json", {"messages": messages, "history": final, "sockets": sockets,
                 "live": [t.live_readiness for t in pair], "old_receiver_instance": old, "transfers": transfers,
                 "passed": passed})
            progress["phase_b"] = passed
            if passed:
                print("T12E_PHASE_B_PASS", flush=True)
            else:
                print("T12E_PHASE_B_NOTIFICATION_INCOMPLETE", flush=True)
    finally:
        for host, term in processes:
            if not (evidence / f"{host.name}-{term.tag}-job.json").exists():
                try:
                    host.receipt(term.tag, descriptor=False, cleanup=True)
                except Exception:
                    missing.append({"host": host.name, "tag": term.tag})
        save(evidence / "result.json", {**progress, "seconds": round(time.monotonic() - started, 3),
             "missing_receipts": missing, "messages": messages, "transfers": transfers, "sockets": sockets,
             "processes": [{"host": h.name, "tag": t.tag, "exit": t.proc.returncode, "quit": t.quit_verified,
                            "incoming": t.events} for h, t in processes]})
    require(progress["phase_a"] and progress["phase_b"] and not missing, "incomplete field proof")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--run-reviewed-pilot", action="store_true")
    arguments = parser.parse_args()
    require(arguments.run_reviewed_pilot, "explicit field-run selection required")
    run(json.loads(arguments.config.read_text(encoding="utf-8-sig")))
