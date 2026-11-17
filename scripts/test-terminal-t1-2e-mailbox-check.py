"""Public fake-child proof for the Windows test mailbox. No product process is launched."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
transport = runpy.run_path(str(HERE / "test-terminal-t1-2e-mailbox.py"))
MailboxClient, frame = (transport[key] for key in ("MailboxClient", "frame"))
Host, require, save, pinned_ssh, remaining = (
    transport["prior"][key] for key in ("Host", "require", "save", "pinned_ssh", "remaining")
)


def eventually(action, end):
    while True:
        if value := action():
            return value
        time.sleep(min(0.15, remaining(end, 0.15)))


def opened(mailbox, end):
    def read():
        try:
            status, rows = mailbox.request(end)
            return (status, rows) if rows or status["terminal"] else None
        except RuntimeError:
            return None
    return eventually(read, end)


def wait_result(mailbox, end, seq, payload):
    def read():
        state, rows = mailbox.request(end, "submit", 0, seq, payload)
        if state["state"] == "done":
            return next(row for row in rows if row.get("id") == f"t12e-{seq}")
        return None
    return eventually(read, end)


def finished(mailbox, end):
    def read():
        reply, rows = mailbox.request(end)
        return (reply, rows) if reply["terminal"] else None
    return eventually(read, end)


def receipt(host, tag, evidence, clean):
    row = host.call("receipt", tag=tag)
    require(row["membership"] and not row["after"] and not row["descriptor"], "owned Job not cleaned")
    if clean:
        require(row["child_exit"] == 0 and row["mailbox_eof"] and row["mailbox_quit"] and not row["forced"] and
                not row["before"] and not row["error"], "unclean mailbox quit")
    else:
        require(row["error"] and not row["mailbox_quit"], "unexpected clean Job")
    save(evidence / f"{tag}-job.json", row)
    return row


def rejects(mailbox, seq, payload, end, generation=None, raw=None):
    cmd = mailbox.host.command("mailbox", tag=mailbox.tag, generation=generation or mailbox.generation,
                               action="submit", sequence=seq, cursor=0,
                               digest=hashlib.sha256(payload).hexdigest())
    if raw is None:
        raw = base64.b64encode(payload) + b"\n"
    result = subprocess.run(cmd, input=raw, capture_output=True, timeout=remaining(end, 15))
    require(result.returncode == 1 and not result.stdout, "invalid command accepted")
    require(b"blindword" not in result.stderr, "private words exposed on reject")


def echo(host, evidence, role):
    tag = "echo"
    mailbox = MailboxClient(host, tag)
    proc = mailbox.owner("mailbox-fake", fake_mode="echo")
    end = time.monotonic() + 108
    try:
        initial, rows = opened(mailbox, end)
        require(rows == [{"v": 1, "type": "ready"}], "echo readiness")
        if role == "pc":
            partial = Path(str(host.setup.parent / "receipts" / "mailbox-echo" / "partial-test"))
            with partial.open("xb") as out:
                out.write(b'{"not-yet-a-complete-frame"')
        time.sleep(3)
        first = frame(1, "echo", {"text": "first"})
        # The first invocation's result is discarded to exercise replay after lost reply.
        mailbox.request(end, "submit", initial["cursor"], 1, first)
        first_result = wait_result(mailbox, end, 1, first)
        require(first_result["value"] == {"count": 1, "text": "first"}, "first delayed frame")
        original, replay = mailbox.request(end, "submit", 0, 1, first)
        require(original["state"] == "done" and replay.count(first_result) == 1, "exact duplicate executed")
        rejects(mailbox, 1, frame(1, "echo", {"text": "conflicting"}), end)
        rejects(mailbox, 2, frame(2, "echo", {}), end, generation="1" * 32)
        rejects(mailbox, 3, frame(3, "echo", {}), end)
        rejects(mailbox, 2, frame(2, "echo", {"text": "x" * 5000}), end)
        rejects(mailbox, 2, frame(2, "echo", {}), end, raw=b"eyJ")
        time.sleep(18)
        second = frame(2, "echo", {"text": "second"})
        second_result = wait_result(mailbox, end, 2, second)
        require(second_result["value"] == {"count": 2, "text": "second"}, "second delayed frame")
        resume, _ = mailbox.request(end)
        third = frame(3, "incoming", {})
        mailbox.request(end, "submit", resume["cursor"], 3, third)

        def delivery():
            state, rows = mailbox.request(end, cursor=resume["cursor"])
            return (state, rows) if len(rows) == 3 else None
        observed, events = eventually(delivery, end)
        require([row["type"] for row in events] == ["result", "event", "event"] and
                [row["text"] for row in events[1:]] == ["first", "second"], "incoming order")
        require(mailbox.request(end, cursor=resume["cursor"])[1] == events and
                mailbox.request(end, cursor=observed["cursor"])[1] == [], "incoming cursor replay")
        last = frame(4, "quit", {})
        require(wait_result(mailbox, end, 4, last)["value"] == {"connected": False}, "echo quit")
        terminal, _ = finished(mailbox, end)
        require(terminal["terminal"] == "clean" and proc.wait(timeout=remaining(end, 5)) == 0,
                "echo wrapper exit")
        require(mailbox.request(end, "submit", 0, 4, last)[0]["state"] == "done", "quit replay")
        rejects(mailbox, 5, frame(5, "echo", {}), end)
        receipt(host, tag, evidence, True)
        save(evidence / "echo.json", {
            "delays_seconds": [3, 18], "first": first_result, "second": second_result,
            "conflict_stale_gap_oversized_partial_rejected": True, "discarded_reply_replayed": True,
            "incoming_cursor": [resume["cursor"], observed["cursor"]], "incoming": events[1:],
            "terminal": terminal["terminal"],
        })
    finally:
        if proc.poll() is None:
            try:
                mailbox.request(time.monotonic() + 10, "stop")
                proc.wait(timeout=10)
            except (RuntimeError, OSError, subprocess.SubprocessError):
                # The owner keeps its absolute Job cutoff.
                pass


PRIVATE_CASES = ("valid", "early", "malformed", "oversized", "truncated", "eof",
                 "timeout", "wrong-id", "wrong-scope", "raw")


def protected(host, evidence, mode):
    tag = "setup-" + mode
    mailbox = MailboxClient(host, tag)
    proc = mailbox.owner("mailbox-fake", fake_mode=mode)
    end = time.monotonic() + 50
    try:
        opened(mailbox, end)
        if mode != "early":
            first = wait_result(mailbox, end, 1, frame(1, "create", {}))
            require(first["value"]["state"] == "recovery_available", "protected fake create")
            second = frame(2, "recovery_show" if mode == "raw" else "save_test_recovery", {})
            mailbox.request(end, "submit", 0, 2, second)
            if mode == "valid":
                require(wait_result(mailbox, end, 2, second)["value"]["state"] == "recovery_saved",
                        "protected recovery projection")
                require(wait_result(mailbox, end, 3, frame(3, "quit", {}))["value"] == {"closed": True},
                        "protected quit")
        result, rows = finished(mailbox, end)
        clean = mode == "valid"
        require(proc.wait(timeout=remaining(end, 6)) == (0 if clean else 1), "protected owner exit")
        require(result["terminal"] == ("clean" if clean else "failed"), "protected terminal")
        public = json.dumps(rows)
        require(all(word not in public for word in ("blindword", "recovery_words", "recovery_scope", "host-recovery")),
                "private material in mailbox outbox")
        if not clean:
            require(not any(row.get("id") == "t12e-2" for row in rows), "invalid response escaped bridge")
        row = receipt(host, tag, evidence, clean)
        require(row["filtered_setup_eof"] == clean, "protected stream EOF result")
        save(evidence / f"{tag}.json", {"terminal": result["terminal"], "public_frames": len(rows)})
    finally:
        if proc.poll() is None:
            try:
                mailbox.request(time.monotonic() + 10, "stop")
                proc.wait(timeout=10)
            except (RuntimeError, OSError, subprocess.SubprocessError):
                pass


def launch_and_abandon(host, evidence, role):
    mailbox = MailboxClient(host, "disconnect")
    proc = mailbox.owner("mailbox-fake", fake_mode="disconnect")
    end = time.monotonic() + 12
    opening, _ = opened(mailbox, end)
    command = frame(1, "hold", {})
    mailbox.request(end, "submit", opening["cursor"], 1, command)

    def started():
        state, rows = mailbox.request(end, cursor=opening["cursor"])
        return state["cursor"] if rows and rows[0]["text"] == "started" else None

    cursor = eventually(started, end)
    if role == "laptop":
        # This Popen owns only the local ssh.exe. The remote PS Job owns the fake child.
        proc.kill()
        proc.wait(timeout=4)
    save(evidence / "controller-left.json", {
        "generation": mailbox.generation, "cursor": cursor, "controller_pid": __import__("os").getpid(),
        "ssh_ended": role == "laptop",
    })


def disconnect(host, evidence, role, argv):
    end = time.monotonic() + 48
    exited = subprocess.run([sys.executable, "-B", *argv, "--abandon"], capture_output=True, timeout=20)
    require(exited.returncode == 0, f"abandon controller failed: {exited.stderr[:1500]!r}")
    left = json.loads((evidence / "controller-left.json").read_bytes())
    mailbox = MailboxClient(host, "disconnect", left["generation"])
    terminal, _ = finished(mailbox, end)
    final, rows = mailbox.request(end, "submit", left["cursor"], 1, frame(1, "hold", {}))
    require(terminal["terminal"] == "failed" and final["state"] == "unknown" and not rows,
            "orphaned command must stay unknown")
    row = receipt(host, "disconnect", evidence, False)
    require(row["forced"] and row["work_expired"], "watchdog did not classify cutoff")
    save(evidence / "disconnect.json", {"controller_exit": exited.returncode, "terminal": terminal["terminal"],
                                         "command_outcome": final["state"], "job_children_after": row["after"]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--role", choices=("pc", "laptop"), required=True)
    parser.add_argument("--attempt", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--abandon", action="store_true")
    args = parser.parse_args()
    config = json.loads(args.config.read_bytes())
    pc, laptop = (config[role]["ipv4"] for role in ("pc", "laptop"))
    argv = pinned_ssh(config["ssh"], pc, laptop) if args.role == "laptop" else []
    host = Host(args.role, config[args.role], pc if argv else laptop, args.attempt, config["pins"], argv,
                args.evidence, time.monotonic() + 395)
    if args.abandon:
        launch_and_abandon(host, args.evidence, args.role)
        return
    args.evidence.mkdir(exist_ok=False)
    save(args.evidence / "init.json", host.call("init"))
    echo(host, args.evidence, args.role)
    disconnect(host, args.evidence, args.role, sys.argv)
    if args.role == "pc":
        with ThreadPoolExecutor(max_workers=3) as pool:
            for future in [pool.submit(protected, host, args.evidence, mode) for mode in PRIVATE_CASES]:
                future.result()
    save(args.evidence / "result.json", {
        "role": args.role, "attempt": args.attempt, "echo": "pass", "disconnect": "pass",
        "protected_cases": list(PRIVATE_CASES) if args.role == "pc" else [],
        "product_runtime_started": False,
    })


if __name__ == "__main__":
    main()
