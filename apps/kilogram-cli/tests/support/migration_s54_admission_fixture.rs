//! Public synthetic metadata only. cfg(test) policy cannot admit a runtime.
use super::*;
pub(super) struct Fixture {
    pub root: PathBuf,
    pub policy: Policy,
    pub input: Inputs,
    pub inventory: Value,
    pub activation: Value,
    pub launch: Value,
    pub capture: Value,
}
pub(super) fn pin(path: &Path) -> Result<Pin> {
    let raw = std::fs::read(path)?;
    Ok(Pin {
        path: path.into(),
        bytes: raw.len() as u64,
        sha256: files::sha(&raw),
    })
}
pub(super) fn write(path: &Path, value: &Value) -> Result<Pin> {
    std::fs::write(path, serde_json::to_vec(value)?)?;
    pin(path)
}
fn public(path: &Path, raw: &[u8]) -> Result<Pin> {
    std::fs::create_dir_all(path.parent().context("fixture parent")?)?;
    std::fs::write(path, raw)?;
    pin(path)
}
fn profile(test: bool) -> Value {
    json!({"opt_level":"3","debuginfo":"line-tables-only","debug_assertions":true,"overflow_checks":true,"test":test})
}
impl Fixture {
    pub fn new() -> Result<Self> {
        let base = workspace().join(".tmp/s54-block4a-r1-01/rust-public-fixtures");
        std::fs::create_dir_all(&base)?;
        let root = tempfile::Builder::new()
            .prefix("public-")
            .tempdir_in(&base)?
            .keep();
        std::fs::create_dir_all(root.join("crates"))?;
        for name in files::HELPERS.into_iter().chain(files::EMBEDDED).chain([
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "apps/kilogram-cli/tests/support/migration_s54_p5.rs",
        ]) {
            public(&root.join(name), b"public fixture source only\n")?;
        }
        public(
            &root.join(".git/HEAD"),
            b"97a5bfb9360a9c3a95dbb4b9f3d84e9b5540f54c\n",
        )?;
        let mut images = Vec::new();
        for role in IMAGES {
            images.push(json!({"role":role.0,"pin":public(&root.join(role.1),format!("public fixture image {}",role.0).as_bytes())?}));
        }
        let config = root.join(".cargo/config");
        std::fs::create_dir_all(config.parent().context("config parent")?)?;
        let configs = vec![Config {
            path: config,
            exists: false,
            sha256: None,
        }];
        let mut resolved = Vec::new();
        for name in [
            "pwsh.exe",
            "powershell.exe",
            "git.exe",
            "rustup.exe",
            "cargo.exe",
            "rustc.exe",
        ] {
            public(
                &root.join("tools").join(name),
                format!("public fixture {name}").as_bytes(),
            )?;
            resolved.push(Resolution {
                name: name.into(),
                path: root.join("tools").join(name),
            });
        }
        let legacy = pin(&root.join(IMAGES[0].1))?;
        let relay = pin(&root.join(IMAGES[2].1))?;
        let policy = Policy {
            root: root.clone(),
            configs: configs.iter().map(|r| r.path.clone()).collect(),
            resolutions: resolved.clone(),
            legacy: (legacy.bytes, legacy.sha256),
            relay: (relay.bytes, relay.sha256),
            fixture: true,
        };
        let source = files::sources(&root)?
            .iter()
            .map(|p| pin(p))
            .collect::<Result<Vec<_>>>()?;
        let source_inputs = source
            .iter()
            .filter_map(|r| {
                let path = r
                    .path
                    .strip_prefix(&root)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                if files::EMBEDDED.contains(&path.as_str())
                    || (files::HELPERS.contains(&path.as_str())
                        && !files::HELPERS[..3].contains(&path.as_str()))
                {
                    None
                } else {
                    Some(json!({"path":path,"sha256":r.sha256}))
                }
            })
            .collect::<Vec<_>>();
        let packages = compiler::REQUIRED
            .iter()
            .map(|n| json!({"id":format!("fixture:{n}"),"name":n}))
            .collect::<Vec<_>>();
        let graph = compiler::REQUIRED
            .iter()
            .map(|n| json!({"id":format!("fixture:{n}"),"features":[],"deps":[]}))
            .collect::<Vec<_>>();
        let tools=["cargo","rustc"].map(|n|{let p=root.join("tools").join(format!("{n}.exe"));let hash=pin(&p)?;Ok(json!({"name":n,"path":p,"sha256":hash.sha256,"shim_path":p,"shim_sha256":hash.sha256}))}).into_iter().collect::<Result<Vec<Value>>>()?;
        let build = json!({"format_version":1,"profile":PROFILE,"target":TARGET,"packages":["kilogram-cli","kilogram-transport-iroh"],"cargo_features":[],
            "tools":tools,"rustc":["rustc 1.98.0","release: 1.98.0",format!("host: {TARGET}")],"cargo":["cargo 1.98.0"],
            "environment":environment()?,"configs":configs,"source_inputs":source_inputs,"package_names":packages,"resolved_graph":graph});
        let evidence = root
            .join(".tmp/migration-s54-public-evidence")
            .join(ATTEMPT);
        std::fs::create_dir_all(&evidence)?;
        let normal = Self::compiler(&root, &build, true)?;
        let tests = Self::compiler(&root, &build, false)?;
        let mut identities = tests["units"].as_array().context("fixture units")?.clone();
        for u in &mut identities {
            u.as_object_mut().context("unit")?.remove("fresh");
        }
        let frozen = json!({"format_version":1,"classification":"frozen-scale-test-artifacts","profile":PROFILE,"target":TARGET,
            "stable_directory":root.join("target/verification-scale/deps"),"build_inputs":build,"units_identity":identities,"artifacts":tests["stable_artifacts"]});
        let embedded = files::EMBEDDED
            .iter()
            .map(|s| pin(&root.join(s)))
            .collect::<Result<Vec<_>>>()?;
        let dep_info = [&tests]
            .into_iter()
            .flat_map(|c| c["units"].as_array().into_iter().flatten())
            .filter_map(|u| u["executable"].as_str())
            .map(|s| pin(&PathBuf::from(s).with_extension("d")))
            .collect::<Result<Vec<_>>>()?;
        let compiler = json!({"normal":write(&evidence.join("normal.json"),&normal)?,"tests":write(&evidence.join("tests.json"),&tests)?,
            "frozen_manifest":write(&evidence.join("frozen.json"),&frozen)?,
            "embedded":{"before":embedded,"after":embedded,"dep_info":dep_info},
            "normal_argv":["build","--package","kilogram-cli","--bin","kilogram-cli","--profile",PROFILE,"--target",TARGET,"--jobs","2","--offline","--locked","--message-format=json-render-diagnostics"],
            "tests_argv":["test","--package","kilogram-cli","--package","kilogram-transport-iroh","--profile",PROFILE,"--target",TARGET,"--jobs","2","--offline","--locked","--no-run","--message-format=json-render-diagnostics"]});
        let run = root.join(".tmp/migration-s54-integrated").join(ATTEMPT);
        std::fs::create_dir_all(&run)?;
        let isolated = json!({"format_version":1,"tests":[{"package":"kilogram-cli","kind":"test","target":"migration_s5_process","name":SELECTOR}]});
        let isolated = write(&evidence.join("isolated.json"), &isolated)?;
        let list = public(
            &evidence.join("list.txt"),
            format!("{SELECTOR}: test\n").as_bytes(),
        )?;
        let actual = std::env::var("TOKIO_WORKER_THREADS").ok();
        let available = std::thread::available_parallelism()?.get();
        let effective = actual
            .as_ref()
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(available);
        let inventory = json!({"schema":1,"classification":"bound_s54_whole_inventory",
            "identity":{"case":CASE,"attempt":ATTEMPT,"selector":SELECTOR,"run_id":state::hex(&[11;32]),"workspace":root,"run":run,
                "output":run.join("assertions.json"),"activation":run.join("activation.json"),"launch":run.join("launch.json"),"capture":run.join("capture.json")},
            "deadline":{"controller":2350,"work":2375,"outer":2400,"origin":"owned_launch","clock":"windows_tick_count64"},
            "source":{"s53_product":S53,"closure_publication":CLOSURE,"implementation":head(&root)?,"observed_head":head(&root)?,"files":source},
            "compiler":compiler,"images":images,"runtime":{"metadata_shell":pin(&resolved[0].path)?,"memory_shell":pin(&resolved[1].path)?,
                "resolutions":resolved,"environment":environment()?,"configs":configs,"workers":{"present":actual.is_some(),"value":actual,"available":available,"effective":effective},
                "cargo_jobs":2,"priority":"BelowNormal","offline":true,"locked":true,"skip_cache":true,"architecture":"x64"},
            "selection":{"isolated":isolated,"list":list,"harness_ref":"stable-migration","named":1,"native_markers":1,"wrapper_markers":1,"aggregates":0}});
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        )?;
        let placeholder = pin(&root.join("Cargo.toml"))?;
        let activation = json!({"schema":1,"classification":"root_s54_whole_activation","inventory":placeholder,"case":CASE,"attempt":ATTEMPT,
            "run_id":state::hex(&[11;32]),"run":run,"selector":SELECTOR,"implementation":head(&root)?,"execution_mode":"NativeToken",
            "deadline":inventory["deadline"],"not_before_utc_ms":now-1000,"not_after_utc_ms":now+3600000,"retry":"root_reallocation_only"});
        let capture = json!({"version":1,"attempt":ATTEMPT,"owner_pid":100,"launcher_pid":101,"harness_path":root.join(IMAGES[5].1),"whole_utc_ms":now+2400000});
        let launch = json!({"schema":1,"classification":"s54_original_launch_binding","inventory":placeholder,"activation":placeholder,"capture":placeholder,
            "case":CASE,"attempt":ATTEMPT,"run_id":state::hex(&[11;32]),"run":run,"owner_pid":100,"launcher_pid":101,
            "start_tick_ms":5000,"start_utc_ms":now,"controller_tick_ms":2355000,"work_tick_ms":2380000,"outer_tick_ms":2405000});
        let input = Inputs {
            inventory: placeholder.clone(),
            activation: placeholder.clone(),
            launch: placeholder.clone(),
            capture: placeholder,
        };
        let mut out = Self {
            root,
            policy,
            input,
            inventory,
            activation,
            launch,
            capture,
        };
        out.seal()?;
        Ok(out)
    }
    fn compiler(root: &Path, build: &Value, normal: bool) -> Result<Value> {
        let mut units = Vec::new();
        let mut raw = Vec::new();
        let mut artifacts = Vec::new();
        let roles = if normal {
            vec![IMAGES[1]]
        } else {
            IMAGES[3..].to_vec()
        };
        let mut entries = roles
            .iter()
            .map(|r| (r.2, r.3, r.4, Some(*r)))
            .collect::<Vec<_>>();
        for name in compiler::REQUIRED {
            if !entries.iter().any(|r| r.0 == name) {
                entries.push((name, "lib", name, None));
            }
        }
        for (index, (package, kind, target, role)) in entries.into_iter().enumerate() {
            let executable = if let Some(r) = role {
                let fixed = root.join(r.1);
                let exe = root.join("target").join(TARGET).join(PROFILE).join(format!(
                    "{}-unit-{index}.exe",
                    if normal { "normal" } else { "tests" }
                ));
                public(&exe, &std::fs::read(&fixed)?)?;
                let dep = exe.with_extension("d");
                let inputs = files::EMBEDDED
                    .into_iter()
                    .chain(["apps/kilogram-cli/tests/support/migration_s54_p5.rs"])
                    .collect::<Vec<_>>()
                    .join(" ");
                public(&dep, format!("{}: {inputs}\n", dep.display()).as_bytes())?;
                let pin = pin(&fixed)?;
                artifacts.push(json!({"package":package,"kind":kind,"target":target,"path":fixed,"bytes":pin.bytes,"sha256":pin.sha256,"profile":PROFILE}));
                Some(exe)
            } else {
                None
            };
            let filename = executable.clone().unwrap_or_else(|| {
                root.join("target")
                    .join(TARGET)
                    .join(PROFILE)
                    .join(format!("unit-{index}.rlib"))
            });
            let u = json!({"package_id":format!("fixture:{package}"),"package":package,"kind":[kind],"target":target,"features":[],"profile":profile(!normal),
                "filenames":[filename],"executable":executable,"class":"runtime-target","fresh":true});
            let msg = json!({"reason":"compiler-artifact","package_id":u["package_id"],"manifest_path":root.join("Cargo.toml"),
                "target":{"kind":[kind],"crate_types":[kind],"name":target,"src_path":root.join("apps/kilogram-cli/tests/support/migration_s54_p5.rs"),
                    "edition":"2024","doc":true,"doctest":false,"test":!normal},
                "profile":u["profile"],"features":[],"filenames":u["filenames"],"executable":u["executable"],"fresh":true});
            raw.push(serde_json::to_string(&msg)?);
            units.push(u);
        }
        Ok(
            json!({"classification":if normal{"nonexecuting-normal-cli-compiler-provenance"}else{"nonexecuting-compiler-artifact-provenance"},
            "profile":PROFILE,"target":TARGET,"expected_manifest_sha256":null,"build_inputs_before":build,"build_inputs_after":build,
            "units":units,"raw_compiler_artifact_json":raw,"stable_artifacts":artifacts,"frozen_guard_verified":false}),
        )
    }
    pub fn seal(&mut self) -> Result<()> {
        // The fixture writer never trusts the deliberately mutated wire run.
        let run = self
            .root
            .join(".tmp/migration-s54-integrated")
            .join(ATTEMPT);
        self.input.inventory = write(&run.join("inputs.json"), &self.inventory)?;
        self.activation["inventory"] = json!(self.input.inventory);
        self.input.activation = write(&run.join("activation.json"), &self.activation)?;
        self.input.capture = write(&run.join("capture.json"), &self.capture)?;
        self.launch["inventory"] = json!(self.input.inventory);
        self.launch["activation"] = json!(self.input.activation);
        self.launch["capture"] = json!(self.input.capture);
        self.input.launch = write(&run.join("launch.json"), &self.launch)?;
        Ok(())
    }
    pub fn inspect(&self) -> Result<Inspection> {
        inspect_with(&self.policy, &self.input)
    }
    pub fn edit_capture(&mut self, normal: bool, edit: impl FnOnce(&mut Value)) -> Result<()> {
        self.try_edit_capture(normal, |value| {
            edit(value);
            Ok(())
        })
    }
    pub fn try_edit_capture(
        &mut self,
        normal: bool,
        edit: impl FnOnce(&mut Value) -> Result<()>,
    ) -> Result<()> {
        let role = if normal { "normal" } else { "tests" };
        let path = PathBuf::from(
            self.inventory["compiler"][role]["path"]
                .as_str()
                .context("capture path")?,
        );
        let mut value: Value = files::json(&std::fs::read(&path)?, 8 * MIB)?;
        edit(&mut value)?;
        self.inventory["compiler"][role] = json!(write(&path, &value)?);
        self.seal()
    }
}
