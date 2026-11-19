"""Short, pinned requests to a test terminal owner. The owner keeps the process pipes."""
import base64
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import time
import uuid


prior = runpy.run_path(str(Path(__file__).with_name("test-terminal-t1-1f-driver.py")))
Terminal, require, remaining, until = (prior[key] for key in ("Terminal", "require", "remaining", "until"))


def frame(sequence, op, args):
    return json.dumps({"v": 1, "id": f"t12e-{sequence}", "op": op, "args": args},
                      ensure_ascii=True, separators=(",", ":")).encode("ascii")


class MailboxClient:
    def __init__(self, host, tag, generation=None):
        self.host = host
        self.tag = tag
        self.generation = generation or uuid.uuid4().hex

    def owner(self, mode, **args):
        argv = self.host.command(mode, tag=self.tag, generation=self.generation, **args)
        return subprocess.Popen(argv, stdin=subprocess.DEVNULL,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def request(self, end, action="read", cursor=0, sequence=0, payload=b""):
        data = base64.b64encode(payload) + b"\n" if action == "submit" else b""
        sha = hashlib.sha256(payload).hexdigest() if action == "submit" else ""
        argv = self.host.command("mailbox", tag=self.tag, generation=self.generation,
                                 action=action, cursor=cursor, sequence=sequence, digest=sha)
        # A bounded retry uses the same sequence and exact bytes after a lost SSH reply.
        for attempt in range(2):
            try:
                seconds = remaining(end, 15)
                require(seconds >= getattr(self.host, 'minimum_subprocess_seconds', 0),
                        'no new tiny-budget mailbox subprocess')
                proc = subprocess.run(argv, input=data, capture_output=True, timeout=seconds)
            except subprocess.TimeoutExpired:
                if attempt:
                    raise
                continue
            if proc.returncode == 255 and not attempt:
                continue
            require(proc.returncode == 0 and not proc.stderr and len(proc.stdout) <= 200000,
                    "mailbox helper failed or rejected request")
            response = json.loads(proc.stdout.decode("utf-8-sig"))
            raw = response["records"]
            require(len(raw) <= 8 and response["cursor"] == cursor + len(raw), "mailbox cursor")
            rows = [base64.b64decode(item, validate=True) for item in raw]
            require(all(0 < len(row) <= 16384 for row in rows), "mailbox public frame cap")
            return response, [json.loads(row) for row in rows]
        raise RuntimeError("mailbox request outcome unknown")


class MailboxTerminal(Terminal):
    def __init__(self, host, tag, mode, deadline, interactive=None, **args):
        self.mailbox = MailboxClient(host, tag)
        self.tag, self.mode, self.deadline = tag, mode, deadline
        self.events = []
        self.last_readiness = self.live_readiness = None
        self.quit_verified = False
        self.seq = self.cursor = 0
        self.queued = []
        self.closed = None
        self.stopped = False
        self.local_owner = interactive.start(self.mailbox, mode, deadline, **args) if interactive else None
        self.proc = None if interactive else self.mailbox.owner(mode, **args)

    def collect(self, end, action="read", sequence=0, payload=b""):
        response, rows = self.mailbox.request(end, action, self.cursor, sequence, payload)
        for doc in rows:
            require(doc["v"] == 1, "mailbox terminal version")
            if doc["type"] == "event":
                require(self.mode != "setup" and doc["event"] == "incoming" and "id" not in doc
                        and prior["HEX_ID"].fullmatch(doc["event_id"]) and len(self.events) < 32,
                        "unexpected terminal event or resync")
                self.events.append({field: doc[field] for field in prior["INCOMING_FIELDS"]})
            else:
                require(doc["type"] in ("ready", "result"), "mailbox terminal frame")
                self.queued.append(doc)
        self.cursor = response["cursor"]
        self.closed = response["terminal"] if len(rows) < 8 else None
        return response

    def line(self, end):
        end = min(end, time.monotonic() + 55)
        while not self.queued:
            if self.closed:
                return None
            require(self.proc is None or self.proc.poll() in (None, 0), "mailbox owner exited")
            try:
                self.collect(end)
            except RuntimeError:
                if self.cursor or (self.proc is not None and self.proc.poll() is not None):
                    raise
                if self.local_owner is not None:
                    self.local_owner.check_alive(end)
                # The initial bounded owner startup can precede the exclusive marker.
                remaining(end, 1)
                time.sleep(0.15)
        return self.queued.pop(0)

    def result(self, op, args, end):
        self.seq += 1
        payload = frame(self.seq, op, args)
        response = self.collect(end, "submit", self.seq, payload)
        require(response["state"] in ("pending", "done"), "mailbox command outcome unknown")
        row = self.line(end)
        require(row and row["type"] == "result" and row["id"] == f"t12e-{self.seq}",
                "mailbox result correlation or closed session")
        return row

    def close(self):
        if self.quit_verified or self.stopped:
            return self.quit_verified
        cleanup_end = time.monotonic() + 35
        previous_deadline = self.deadline
        self.deadline = min(cleanup_end, time.monotonic() + 25)
        try:
            self.quit()
            return True
        except (RuntimeError, OSError, subprocess.SubprocessError):
            # A protocol failure still needs the existing bounded stop fallback.
            pass
        finally:
            self.deadline = previous_deadline
        self.stopped = True
        if self.local_owner is not None:
            try:
                self.collect(min(cleanup_end, time.monotonic() + 12), "stop")
            except (RuntimeError, OSError, subprocess.SubprocessError):
                pass  # The local watchdog remains authoritative if stop cannot be confirmed.
            self.local_owner.wait(min(cleanup_end, time.monotonic() + 18))
            return False
        if self.proc.poll() is None:
            try:
                self.collect(min(cleanup_end, time.monotonic() + 12), "stop")
            except (RuntimeError, OSError, subprocess.SubprocessError):
                # The owner's independent deadline remains armed if SSH is unavailable.
                pass
        try:
            self.proc.wait(timeout=remaining(cleanup_end, 18))
        except subprocess.TimeoutExpired:
            # This is only the controller's owned launcher. The host Job owns the child.
            self.proc.kill()
            self.proc.wait(timeout=4)
        return False

    def quit(self):
        def check(end):
            result = self.result("quit", {}, end)
            if self.mode == "chat" and result.get("code") == "session_closing":
                return None
            expected = {"closed": True} if self.mode == "setup" else {"connected": False}
            require(result["ok"] and result["value"] == expected, "mailbox quit result")
            return True
        until(check, 25, self.deadline, "mailbox quit")
        end = min(self.deadline, time.monotonic() + 25)
        while self.closed is None:
            self.collect(end)
        if not self.queued:
            # Fetch any last records beyond the last bounded page.
            while self.collect(end)["records"]:
                pass
        require(self.closed == "clean" and not self.queued, "mailbox quit or streams failed")
        limit = remaining(end, 12)
        exit_code = (self.local_owner.wait(time.monotonic() + limit) if self.local_owner is not None
                     else self.proc.wait(timeout=limit))
        require(exit_code == 0, "mailbox owner exit differs")
        self.quit_verified = True
