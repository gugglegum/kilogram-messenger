//! One opt-in lifecycle ledger. The required runner input has no default;
//! full whole-run schema/admission/result composition remains outside this seam.
use super::*;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, ffi::OsStr, marker::PhantomData, rc::Rc};
#[path = "migration_s54_memory_scope_tests.rs"]
pub(crate) mod tests;
#[derive(Clone, Serialize)]
pub(crate) struct Pin {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}
impl Pin {
    pub(crate) fn verify(&self) -> Result<()> {
        ensure!(
            self.path.is_absolute() && self.bytes > 0 && self.sha256.len() == 64,
            "memory runner pin"
        );
        let m = std::fs::symlink_metadata(&self.path)?;
        ensure!(
            m.is_file() && !m.file_type().is_symlink() && m.len() == self.bytes,
            "memory runner regular file"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(m.file_attributes() & 0x400 == 0, "memory runner reparse");
        }
        use std::io::Read;
        let mut f = std::fs::File::open(&self.path)?;
        let mut hash = Sha256::new();
        let mut buf = [0; 8192];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        ensure!(
            format!("{:x}", hash.finalize()) == self.sha256,
            "memory runner pin changed"
        );
        Ok(())
    }
}
/// The future mandatory runner supplies reviewed pins and binding, not an env
/// success bit. Binding opaque inventory bytes does NOT validate its whole schema.
pub(crate) struct RequiredInput {
    pub run: Id,
    pub attempt: String,
    pub parent: PathBuf,
    pub inventory: Pin,
    pub images: [Pin; 2],
    pub shell: query::Shell,
}
struct Input {
    required: RequiredInput,
    clock: s53_deadline::ObserverContext,
    synthetic: bool,
}
thread_local! {static INPUT:RefCell<Option<Input>>=const{RefCell::new(None)};
static SLOT:RefCell<Slot>=const{RefCell::new(Slot{state:None,busy:false,poison:false})};}
pub(crate) struct InputGuard(PhantomData<Rc<()>>);
impl InputGuard {
    pub(crate) fn install(required: RequiredInput) -> Result<Self> {
        Self::install_kind(required, false)
    }
    fn install_kind(required: RequiredInput, synthetic: bool) -> Result<Self> {
        ensure!(!active(), "one memory input/scope");
        let clock = s53_deadline::observer_context()?;
        ensure!(
            required.run != [0; 32]
                && !required.attempt.is_empty()
                && required
                    .parent
                    .canonicalize()?
                    .starts_with(workspace().canonicalize()?.join(".tmp")),
            "memory required run binding"
        );
        required.inventory.verify()?;
        required.shell.verify()?;
        for p in &required.images {
            p.verify()?;
        }
        if !synthetic {
            ensure!(
                required.images[0].path.canonicalize()? == frozen().canonicalize()?
                    && required.images[1].path.canonicalize()? == current().canonicalize()?,
                "memory required runtime images"
            );
        }
        INPUT.with_borrow_mut(|slot| {
            ensure!(slot.is_none(), "one memory runner input");
            *slot = Some(Input {
                required,
                clock,
                synthetic,
            });
            Ok(Self(PhantomData))
        })
    }
}
impl Drop for InputGuard {
    fn drop(&mut self) {
        INPUT.with_borrow_mut(|s| *s = None);
    }
}
struct Slot {
    state: Option<State>,
    busy: bool,
    poison: bool,
}
struct Lease(Option<State>);
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(mut state) = self.0.take() {
            SLOT.with_borrow_mut(|s| {
                if s.poison && !state.finished {
                    state.fail();
                }
                s.state = Some(state);
                s.busy = false;
                s.poison = false;
            });
        }
    }
}
fn eval<T>(read: bool, f: impl FnOnce(&mut State) -> Result<T>) -> Result<Option<T>> {
    let state = SLOT.with_borrow_mut(|s| {
        if s.busy {
            s.poison = true;
            anyhow::bail!("memory registry reentry");
        }
        if let Some(state) = s.state.take() {
            s.busy = true;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    })?;
    let Some(state) = state else { return Ok(None) };
    let mut lease = Lease(Some(state));
    let s = lease.0.as_mut().context("memory registry lease")?;
    if s.finished && !read {
        anyhow::bail!("memory finalized integration is immutable");
    }
    let result = f(s);
    if result.is_err() && !s.finished {
        s.fail();
    }
    result.map(Some)
}
pub(crate) fn active() -> bool {
    SLOT.with_borrow(|s| s.busy || s.state.is_some())
}
pub(crate) struct Guard(PhantomData<Rc<()>>);
impl Guard {
    pub(crate) fn enter(parent: &Path, until: Instant) -> Result<Self> {
        ensure!(!active(), "one memory observer scope");
        let attempt = s54_capture::admitted_attempt()?;
        let input = INPUT
            .with_borrow_mut(Option::take)
            .context("required memory runner descriptor absent")?;
        let clock = s53_deadline::observer_context()?;
        ensure!(
            clock.origin == input.clock.origin
                && clock.whole == input.clock.whole
                && clock.run.canonicalize()? == input.clock.run.canonicalize()?
                && parent.canonicalize()? == input.required.parent.canonicalize()?
                && attempt == input.required.attempt
                && Instant::now() < until.min(clock.whole),
            "same original memory run/admission/clock"
        );
        input.required.inventory.verify()?;
        input.required.shell.verify()?;
        let ledger = Ledger::with_origin(input.required.run, &clock)?;
        SLOT.with_borrow_mut(|s| {
            s.state = Some(State {
                ledger,
                input,
                roots: [None, None],
                sources: [None, None],
                aliases: vec![],
                raw: vec![],
                phase: Phase::Setup,
                finished: false,
                output: Some(clock.run),
                queries: 0,
            })
        });
        Ok(Self(PhantomData))
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let state = SLOT.with_borrow_mut(|s| s.state.take());
        if let Some(mut s) = state
            && !s.finished
        {
            s.fail();
            let _ = s.write("memory-observer-abort.json");
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
struct Tag {
    pid: u32,
    #[serde(skip)]
    handle: usize,
}
pub(crate) fn handle_tag(child: &Child) -> Result<usize> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        Ok(child.as_raw_handle() as usize)
    }
    #[cfg(not(windows))]
    {
        let _ = child;
        anyhow::bail!("memory Windows handle required")
    }
}
fn tag(child: &Child) -> Result<Tag> {
    Ok(Tag {
        pid: child.id(),
        handle: handle_tag(child)?,
    })
}
#[derive(Clone, Serialize)]
struct Alias {
    owner: u8,
    root: PathBuf,
    profile: PathBuf,
    ipc: PathBuf,
    current: bool,
}
#[derive(Clone)]
pub(crate) struct Prepared {
    alias: Alias,
    config: String,
    image: Id,
}
#[derive(Serialize)]
struct Raw {
    tag: Tag,
    alias: Alias,
    config: String,
    image: Id,
    token: Option<Token>,
    binding: Option<Binding>,
    waited: bool,
    code: Option<i32>,
    stop: Option<Stop>,
    aborted: bool,
}
pub(crate) struct Ticket {
    ordinal: usize,
    token: Token,
    tag: Tag,
}
struct State {
    ledger: Ledger,
    input: Input,
    roots: [Option<PathBuf>; 2],
    sources: [Option<Id>; 2],
    aliases: Vec<Alias>,
    raw: Vec<Raw>,
    phase: Phase,
    finished: bool,
    output: Option<PathBuf>,
    queries: usize,
}
fn canonical_file(path: &Path) -> Result<PathBuf> {
    Ok(path.canonicalize()?)
}
fn ipc_key(path: &Path) -> Result<PathBuf> {
    Ok(path
        .parent()
        .context("memory IPC parent")?
        .canonicalize()?
        .join(path.file_name().context("memory IPC name")?))
}
impl State {
    fn fail(&mut self) {
        if !self.finished && !self.ledger.sealed {
            self.ledger.failed = true;
        }
    }
    fn healthy(&self) -> Result<()> {
        ensure!(
            !self.ledger.failed && !self.finished,
            "memory observer failed/finished"
        );
        Ok(())
    }
    fn alias(
        &mut self,
        owner: u8,
        root: &Path,
        profile: &Path,
        ipc: &Path,
        current: bool,
    ) -> Result<()> {
        self.healthy()?;
        ensure!(owner < 2, "memory owner role");
        let root = root.canonicalize()?;
        ensure!(
            root.starts_with(self.input.required.parent.canonicalize()?)
                && root != self.input.required.parent.canonicalize()?,
            "memory original directory"
        );
        let i = usize::from(owner);
        ensure!(
            self.roots[i].as_ref().is_none_or(|r| *r == root)
                && self.roots[1 - i].as_ref().is_none_or(|r| *r != root),
            "memory original root identity"
        );
        self.roots[i] = Some(root.clone());
        let a = Alias {
            owner,
            root,
            profile: canonical_file(profile)?,
            ipc: ipc_key(ipc)?,
            current,
        };
        if let Some(old) = self.aliases.iter_mut().find(|v| v.profile == a.profile) {
            ensure!(
                old.owner == owner && old.root == a.root,
                "memory foreign profile alias"
            );
            *old = a;
        } else {
            ensure!(self.aliases.len() < 4, "memory alias bound");
            self.aliases.push(a);
        }
        Ok(())
    }
    fn prepare(&mut self, profile: &Path, exe: &Path) -> Result<Prepared> {
        self.healthy()?;
        ensure!(self.raw.len() < LIVES, "memory actual spawn bound");
        let a = self
            .aliases
            .iter()
            .find(|a| canonical_file(profile).is_ok_and(|p| p == a.profile))
            .context("unmapped actual resident profile")?
            .clone();
        ensure!(
            !self
                .raw
                .iter()
                .any(|r| !r.waited && r.alias.owner == a.owner),
            "memory overlapping owner before spawn"
        );
        let image = &self.input.required.images[usize::from(a.current)];
        ensure!(
            canonical_file(exe)? == canonical_file(&image.path)?,
            "memory actual runtime image"
        );
        image.verify()?;
        let bytes = std::fs::read(&a.profile)?;
        ensure!(bytes.len() <= 64 * 1024, "memory profile provenance cap");
        Ok(Prepared {
            alias: a,
            config: format!("{:x}", Sha256::digest(bytes)),
            image: kilogram_cli::ordered_dialog::state::parse_id(&image.sha256)?,
        })
    }
    fn seen(&mut self, tag: Tag, p: Prepared) -> Result<usize> {
        ensure!(self.raw.len() < LIVES, "memory actual spawn bound");
        let overlap = self
            .raw
            .iter()
            .any(|r| !r.waited && (r.tag == tag || r.alias.owner == p.alias.owner));
        let i = self.raw.len();
        self.raw.push(Raw {
            tag,
            alias: p.alias,
            config: p.config,
            image: p.image,
            token: None,
            binding: None,
            waited: false,
            code: None,
            stop: None,
            aborted: false,
        });
        // Even a rejected registration retains the actual spawn and allows
        // Drop to record its cleanup independently of observer tokens.
        if overlap {
            self.fail();
            anyhow::bail!("memory actual spawn overlap");
        }
        Ok(i)
    }
    fn identity_register(&mut self, i: usize) -> Result<Token> {
        self.healthy()?;
        let r = self.raw.get(i).context("memory actual spawn missing")?;
        ensure!(
            r.token.is_none() && !r.waited,
            "memory duplicate registration"
        );
        let profile =
            *blake3::hash(format!("{}:{}", r.alias.root.display(), r.alias.owner).as_bytes())
                .as_bytes();
        let t = self.ledger.register_at(
            self.ledger.now_ms(),
            Profile {
                owner: r.alias.owner,
                profile,
                image: r.image,
                current: r.alias.current,
                pid: r.tag.pid,
            },
        )?;
        self.raw[i].token = Some(t);
        if !self.raw[i].alias.current
            && let Some(source) = self.sources[usize::from(self.raw[i].alias.owner)]
        {
            self.bind_one(i, source, None)?;
        }
        Ok(t)
    }
    fn bind_one(&mut self, i: usize, source: Id, nonce: Option<Id>) -> Result<()> {
        let r = &self.raw[i];
        let t = r.token.context("memory binding requires registration")?;
        let b = Binding {
            owner: r.alias.owner,
            profile: self.ledger.lives[t.ordinal].profile.profile,
            source,
            nonce,
        };
        if let Some(old) = r.binding {
            ensure!(
                old.source == b.source && old.nonce == b.nonce && old.owner == b.owner,
                "changed verified memory incarnation"
            );
            return Ok(());
        }
        self.ledger.bind(t, b)?;
        self.raw[i].binding = Some(b);
        Ok(())
    }
    fn find(&self, tag: Tag) -> Option<usize> {
        self.raw.iter().position(|r| r.tag == tag && !r.waited)
    }
    fn sample(&mut self, i: usize, reason: Reason, child: &mut Child) -> Result<()> {
        self.healthy()?;
        let r = &self.raw[i];
        ensure!(r.tag == tag(child)?, "memory foreign actual child");
        let t = r.token.context("memory omitted observer registration")?;
        self.queries += 1;
        self.ledger
            .sample(t, reason, self.phase, child, &self.input.required.shell)
    }
    fn checkpoint(&mut self, i: usize, child: &mut Child) -> Result<()> {
        self.healthy()?;
        ensure!(
            self.raw[i].tag == tag(child)?,
            "memory foreign checkpoint handle"
        );
        self.ledger.live_checkpoint(
            self.raw[i].token.context("memory omitted registration")?,
            child,
        )
    }
    fn progress(&mut self, child: &mut Child) -> Result<()> {
        let Some(i) = self.find(tag(child)?) else {
            return Ok(());
        };
        self.healthy()?;
        if self.raw[i].stop.is_some() {
            return Ok(());
        }
        self.checkpoint(i, child)?;
        let t = self.raw[i].token.context("memory missing token")?;
        if self.ledger.progress_due_at(t, self.ledger.now_ms())? {
            self.sample(i, Reason::Progress, child)?;
        }
        Ok(())
    }
    fn before(&mut self, child: &mut Child, kind: Stop) -> Result<()> {
        let Some(i) = self.find(tag(child)?) else {
            return Ok(());
        };
        let observed = self
            .checkpoint(i, child)
            .and_then(|_| self.sample(i, Reason::BeforeStop, child));
        self.raw[i].stop = Some(kind);
        let close = self.ledger.request_stop_at(
            self.raw[i].token.context("memory missing stop token")?,
            kind,
            self.ledger.now_ms(),
        );
        observed.and(close)
    }
    fn terminal(
        &mut self,
        child: &Child,
        waited: bool,
        code: Option<i32>,
        aborted: bool,
    ) -> Result<()> {
        self.terminal_tag(tag(child)?, waited, code, aborted)
    }
    fn terminal_tag(
        &mut self,
        child_tag: Tag,
        waited: bool,
        code: Option<i32>,
        aborted: bool,
    ) -> Result<()> {
        let Some(i) = self.find(child_tag) else {
            ensure!(
                !self.raw.iter().any(|r| r.tag == child_tag),
                "duplicate actual memory terminal"
            );
            return Ok(());
        };
        self.raw[i].waited = waited;
        self.raw[i].code = code;
        self.raw[i].aborted |= aborted;
        if aborted {
            self.fail();
        }
        if let Some(t) = self.raw[i].token {
            self.ledger
                .close_at(t, self.ledger.now_ms(), waited, code)?;
        } else {
            self.fail();
            anyhow::bail!("actual resident omitted from memory observer");
        }
        Ok(())
    }
    fn ticket(&self, profile: &Path, ipc: &Path, current: bool) -> Result<Option<Ticket>> {
        self.healthy()?;
        let p = canonical_file(profile)?;
        let ipc = ipc_key(ipc)?;
        let (i, r) = self
            .raw
            .iter()
            .enumerate()
            .find(|(_, r)| !r.waited && r.alias.profile == p && r.alias.ipc == ipc)
            .context("status requires actual live resident")?;
        ensure!(
            r.alias.current == current && r.stop.is_none(),
            "status closing/format mismatch"
        );
        if !current {
            return Ok(None);
        }
        Ok(Some(Ticket {
            ordinal: i,
            token: r.token.context("status omitted registration")?,
            tag: r.tag,
        }))
    }
    fn candidate(&mut self, t: Ticket, v: &Value) -> Result<()> {
        self.healthy()?;
        let r = self.raw.get(t.ordinal).context("foreign status lifetime")?;
        ensure!(
            !r.waited && r.stop.is_none() && r.tag == t.tag && r.token == Some(t.token),
            "stale/cross-restart status"
        );
        self.ledger.candidate_at(t.token, self.ledger.now_ms(), v)
    }
    fn actual_complete(&self) -> bool {
        self.roots.iter().all(Option::is_some)
            && self.sources.iter().all(Option::is_some)
            && (0..2).all(|owner| self.raw.iter().any(|r| usize::from(r.alias.owner) == owner))
            && !self.raw.is_empty()
            && self.raw.len() == self.ledger.lives.len()
            && self
                .raw
                .iter()
                .all(|r| r.token.is_some() && r.waited && r.binding.is_some() && !r.aborted)
    }
    fn report(&self) -> Value {
        json!({"classification":"memory observation only; whole-run admission/composition NOT implemented",
        "original_lifetime_coverage_complete":self.actual_complete(),
        "synthetic_fixture":self.input.synthetic,"inventory":self.input.required.inventory,"actual_resident_spawns":self.raw.len(),
        "actual_terminal_outcomes":self.raw.iter().filter(|r|r.waited).count(),"query_attempts":self.queries,"actual_lifetimes":self.raw,
        "coverage":self.ledger.diagnostic(),"rows":self.ledger.rows(),"finished":self.finished})
    }
    fn write(&self, name: &str) -> Result<()> {
        if let Some(dir) = &self.output {
            use std::io::Write;
            let bytes = serde_json::to_vec(&self.report())?;
            ensure!(bytes.len() <= 2 * 1024 * 1024, "memory diagnostic bound");
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join(name))?
                .write_all(&bytes)?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<Value> {
        self.healthy()?;
        ensure!(
            self.actual_complete(),
            "actual spawn/registration/terminal/original-owner coverage mismatch"
        );
        self.ledger.finish()?;
        self.finished = true;
        if let Err(e) = self.write("memory-observer-final.json") {
            self.finished = false;
            return Err(e);
        }
        Ok(self.report())
    }
}
pub(crate) fn owner(
    owner: u8,
    root: &Path,
    profile: &Path,
    ipc: &Path,
    current: bool,
) -> Result<()> {
    eval(false, |s| s.alias(owner, root, profile, ipc, current)).map(|_| ())
}
pub(crate) fn alias(root: &Path, profile: &Path, ipc: &Path, current: bool) -> Result<()> {
    eval(false, |s| {
        let root = root.canonicalize()?;
        let i = s
            .roots
            .iter()
            .position(|r| r.as_ref() == Some(&root))
            .context("memory original owner directory missing")?;
        s.alias(i as u8, &root, profile, ipc, current)
    })
    .map(|_| ())
}
pub(crate) fn prepare(cmd: &Command) -> Result<Option<Prepared>> {
    Ok(eval(false, |s| {
        let a: Vec<_> = cmd.get_args().collect();
        if a.first()
            .is_none_or(|v| *v != OsStr::new("runtime-from-profile"))
        {
            return Ok(None);
        }
        ensure!(
            a.len() == 3 && a[1] == OsStr::new("--profile-file"),
            "memory structured runtime command"
        );
        s.prepare(Path::new(a[2]), Path::new(cmd.get_program()))
            .map(Some)
    })?
    .flatten())
}
pub(crate) fn spawned(child: &Child, p: Prepared) -> Result<usize> {
    eval(false, |s| s.seen(tag(child)?, p))?.context("memory scope lost during actual spawn")
}
pub(crate) fn registered(child: &mut Child, i: usize) -> Result<()> {
    eval(false, |s| {
        s.identity_register(i)?;
        s.sample(i, Reason::Start, child)
    })
    .map(|_| ())
}
pub(crate) fn progress(child: &mut Child) -> Result<()> {
    eval(false, |s| s.progress(child)).map(|_| ())
}
pub(crate) fn checkpoint(child: &mut Child) -> Result<()> {
    eval(false, |s| {
        if let Some(i) = s.find(tag(child)?) {
            s.checkpoint(i, child)?;
        }
        Ok(())
    })
    .map(|_| ())
}
#[cfg(test)]
pub(crate) fn prepare_fixture(cmd: &Command, profile: &Path) -> Result<Prepared> {
    eval(false, |s| {
        ensure!(
            s.input.synthetic,
            "fixture requires explicit synthetic descriptor"
        );
        s.prepare(profile, Path::new(cmd.get_program()))
    })?
    .context("synthetic observer scope required")
}
pub(crate) fn before_kill(child: &mut Child) -> Result<()> {
    eval(false, |s| s.before(child, Stop::Kill)).map(|_| ())
}
pub(crate) fn before_normal(child: &mut Child) -> Result<()> {
    eval(false, |s| s.before(child, Stop::Normal)).map(|_| ())
}
pub(crate) fn terminal(
    child: &Child,
    waited: bool,
    code: Option<i32>,
    aborted: bool,
) -> Result<()> {
    eval(false, |s| s.terminal(child, waited, code, aborted)).map(|_| ())
}
pub(crate) fn dropping(child: &Child) {
    let _ = eval(false, |s| {
        if s.find(tag(child)?).is_some() {
            s.fail();
        }
        Ok(())
    });
}
pub(crate) fn observed(child: &Child) -> bool {
    eval(true, |s| Ok(s.find(tag(child)?).is_some()))
        .ok()
        .flatten()
        .unwrap_or(false)
}
pub(crate) fn phase(n: u8) -> Result<()> {
    eval(false, |s| {
        s.healthy()?;
        ensure!(n <= 12, "memory phase bound");
        s.phase = Phase::Stage(n);
        Ok(())
    })
    .map(|_| ())
}
pub(crate) fn bind_legacy(owner: u8, source: Id) -> Result<()> {
    eval(false, |s| {
        ensure!(
            owner < 2
                && source != [0; 32]
                && s.sources[usize::from(owner)].is_none_or(|old| old == source),
            "memory original source changed"
        );
        s.sources[usize::from(owner)] = Some(source);
        for i in 0..s.raw.len() {
            if s.raw[i].alias.owner == owner
                && !s.raw[i].alias.current
                && s.raw[i].binding.is_none()
            {
                s.bind_one(i, source, None)?;
            }
        }
        Ok(())
    })
    .map(|_| ())
}
pub(crate) fn bind_current(child: &Child, source: Id, nonce: Id) -> Result<()> {
    eval(false, |s| {
        let Some(i) = s.find(tag(child)?) else {
            return Ok(());
        };
        if !s.raw[i].alias.current {
            return Ok(());
        }
        ensure!(
            s.sources[usize::from(s.raw[i].alias.owner)] == Some(source),
            "memory decoder original source"
        );
        s.bind_one(i, source, Some(nonce))
    })
    .map(|_| ())
}
pub(crate) fn around_status(
    profile: &Path,
    ipc: &Path,
    current: bool,
    f: impl FnOnce() -> Result<Value>,
) -> Result<Value> {
    let ticket = eval(false, |s| s.ticket(profile, ipc, current))?.flatten();
    let value = match f() {
        Ok(v) => v,
        Err(e) => {
            let _ = eval(false, |s| {
                s.fail();
                Ok(())
            });
            return Err(e);
        }
    };
    if let Some(t) = ticket {
        eval(false, |s| s.candidate(t, &value))?;
    }
    Ok(value)
}
pub(crate) fn diagnostic() -> Result<Value> {
    eval(true, |s| Ok(s.report()))?.context("memory observer absent")
}
pub(crate) fn finish_coverage() -> Result<Value> {
    eval(false, State::finish)?.context("memory observer absent")
}
