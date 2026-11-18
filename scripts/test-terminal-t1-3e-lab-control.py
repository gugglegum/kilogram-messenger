"""Pinned ordinary SSH controller for the reusable lab agent (no firewall mutation)."""
import argparse
import base64
import json
from pathlib import Path, PureWindowsPath
import subprocess
import time
import uuid


class Agent:
    def __init__(self, prefix, base, script_sha, config_sha):
        self.prefix = list(prefix)  # Exact pinned SSH argv, or local PowerShell in focused checks.
        self.base = PureWindowsPath(base)
        self.script_sha, self.config_sha = script_sha, config_sha
        self.session = None

    def invoke(self, action, request=None, timeout=17):
        script = self.base / 'ordinary-agent/test-terminal-t1-3e-lab-agent.ps1'
        config = self.base / 'ordinary-agent/lab.json'
        def literal(s):
            return "'" + str(s).replace("'", "''") + "'"
        for pin in (self.script_sha, self.config_sha):
            if len(pin) != 64 or any(c not in '0123456789abcdef' for c in pin):
                raise ValueError('lab pin')
        code = "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';"
        code += 'Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1";'
        for path, pin in ((script, self.script_sha), (config, self.config_sha)):
            code += f"if((Get-FileHash -LiteralPath {literal(path)}).Hash -ine '{pin}'){{throw 'LAB_PIN'}};"
        if action == 'ready':
            code += f"[IO.File]::ReadAllText({literal(self.base / 'ordinary-agent/control/ready.json')})"
        else:
            code += f'& {literal(script)} -Action {action} -Config {literal(config)}'
            if request is not None:
                code += ' -Request ' + literal(json.dumps(request, separators=(',', ':'), ensure_ascii=True))
        encoded = base64.b64encode(code.encode('utf-16-le')).decode()
        if len(encoded) >= 7400:
            raise ValueError('lab SSH command length')
        p = subprocess.run(self.prefix + ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', encoded],
                           capture_output=True, timeout=timeout)
        if p.returncode or p.stderr or not 0 < len(p.stdout) <= 131072:
            raise RuntimeError(f'lab transport outcome unconfirmed (exit {p.returncode}): {p.stderr[:500]!r}')
        return json.loads(p.stdout.decode('utf-8-sig'))

    def ready(self):
        row = self.invoke('ready')
        if (row['v'] != 1 or row['administrator'] is not False or row['logon_type'] != 2
                or row['windows_session'] <= 0 or row['profile_hive_loaded'] is not True):
            raise RuntimeError('lab interactive availability/context')
        self.session = row['session']
        return row

    def request(self, op, **args):
        if self.session is None:
            raise RuntimeError('read ready first')
        return {'v': 1, 'session': self.session, 'id': uuid.uuid4().hex, 'op': op, 'args': args}

    def exchange(self, request):
        # Lost-reply retries must use this same immutable request, never create a new begin.
        row = self.invoke('Command', request)
        if (row['v'], row['id'], row['session']) != (1, request['id'], request['session']):
            raise RuntimeError('lab response identity')
        if not row['ok']:
            raise RuntimeError(row['error'])
        return row['value']

    def call(self, op, **args):
        return self.exchange(self.request(op, **args))

    def wait(self, attempt, seconds=30):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            row = self.call('status', attempt=attempt)
            if row['result'] is not None and row['active'] != attempt:
                return row['result']
            if row['blocked']:
                raise RuntimeError(row['blocked'])
            time.sleep(.15)
        raise TimeoutError('lab attempt cleanup not confirmed')


class Attempt:
    """Owner adapter for the existing MailboxTerminal; the ordinary agent starts the owner."""
    def __init__(self, agent, attempt, generation):
        self.agent, self.attempt, self.generation = agent, attempt, generation

    def start(self, mailbox, mode, end, **args):
        if mode not in ('setup', 'chat') or set(args) - {'fake_mode', 'observe_socket'}:
            raise ValueError('fixed owner request')
        if 'observe_socket' in args and (mode != 'chat' or args['observe_socket'] is not True):
            raise ValueError('fixed socket observation')
        self.agent.call('owner', attempt=self.attempt, generation=self.generation, tag=mailbox.tag,
                        owner_generation=mailbox.generation, mode=mode, fake_mode=args.get('fake_mode', ''))
        return Owner(self, mailbox.tag, mailbox.generation)


class Owner:
    def __init__(self, attempt, tag, generation):
        self.attempt, self.tag, self.generation = attempt, tag, generation
        self.process = None

    def sample(self):
        row = self.attempt.agent.call('status', attempt=self.attempt.attempt)
        owner = None
        for item in row['owners']:
            if item and item['tag'] == self.tag:
                if item['generation'] != self.generation:
                    raise RuntimeError('owner generation changed')
                identity = item['worker_pid'], item['worker_start']
                if not all(type(n) is int and n > 0 for n in identity) or self.process not in (None, identity):
                    raise RuntimeError('owner incarnation changed')
                self.process = identity
                owner = item
        return row, owner

    def check_alive(self, end):
        row, owner = self.sample()
        if row['result'] is not None or (owner and owner['state'] != 'running'):
            raise RuntimeError('owner failed before mailbox readiness')

    def wait(self, end):
        while time.monotonic() < end:
            row, owner = self.sample()
            if owner and owner['state'] == 'clean' and owner['exit'] == 0:
                return 0
            if row['result'] is not None or (owner and owner['state'] == 'failed'):
                raise RuntimeError('owner clean exit unconfirmed')
            time.sleep(.1)
        raise TimeoutError('owner cleanup deadline')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('manifest', type=Path, help='Reviewed JSON: prefix, base, script_sha, config_sha')
    p.add_argument('operation', choices=('ready', 'wake', 'request'))
    p.add_argument('--request', type=Path, help='Exact immutable request JSON; retain it for unknown outcomes')
    args = p.parse_args()
    agent = Agent(**json.loads(args.manifest.read_text(encoding='utf-8')))
    if args.operation == 'ready':
        row = agent.ready()
    elif args.operation == 'wake':
        row = agent.invoke('Wake')
    else:
        row = agent.exchange(json.loads(args.request.read_text(encoding='utf-8')))
    print(json.dumps(row, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
