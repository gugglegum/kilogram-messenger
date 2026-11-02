//! Closed public wire types. Nullable fields are required, never defaults.
use super::*;
use serde::{Deserialize, Deserializer, Serialize};
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
macro_rules! wire {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
        #[serde(deny_unknown_fields)]
        pub(in super::super) struct $name { $($fields)* }
    };
}
wire!(Pin { pub path: PathBuf, pub bytes: u64, pub sha256: String });
wire!(Identity {
    pub case: String, pub attempt: String, pub selector: String, pub run_id: String,
    pub workspace: PathBuf, pub run: PathBuf, pub output: PathBuf,
    pub activation: PathBuf, pub launch: PathBuf, pub capture: PathBuf,
});
wire!(Deadline { pub controller: u64, pub work: u64, pub outer: u64, pub origin: String, pub clock: String });
wire!(Source {
    pub s53_product: String, pub closure_publication: String,
    pub implementation: String, pub observed_head: String, pub files: Vec<Pin>,
});
wire!(Image { pub role: String, pub pin: Pin });
wire!(Environment {
    pub name: String, pub present: bool,
    #[serde(deserialize_with="nullable")] pub value_sha256: Option<String>,
});
wire!(Config {
    pub path: PathBuf, pub exists: bool,
    #[serde(deserialize_with="nullable")] pub sha256: Option<String>,
});
wire!(BuildTool { pub name: String, pub path: PathBuf, pub sha256: String, pub shim_path: PathBuf, pub shim_sha256: String });
wire!(SourceDigest { pub path: String, pub sha256: String });
wire!(Package { pub id: String, pub name: String });
wire!(DepKind {
    #[serde(deserialize_with="nullable")] pub kind: Option<String>,
    #[serde(deserialize_with="nullable")] pub target: Option<String>,
});
wire!(Dependency { pub name: String, pub pkg: String, pub dep_kinds: Vec<DepKind> });
wire!(Graph { pub id: String, pub features: Vec<String>, pub deps: Vec<Dependency> });
wire!(BuildInputs {
    pub format_version: u32, pub profile: String, pub target: String,
    pub packages: Vec<String>, pub cargo_features: Vec<String>, pub tools: Vec<BuildTool>,
    pub rustc: Vec<String>, pub cargo: Vec<String>, pub environment: Vec<Environment>,
    pub configs: Vec<Config>, pub source_inputs: Vec<SourceDigest>,
    pub package_names: Vec<Package>, pub resolved_graph: Vec<Graph>,
});
wire!(Profile {
    pub opt_level: String, pub debuginfo: Value,
    pub debug_assertions: bool, pub overflow_checks: bool, pub test: bool,
});
wire!(Unit {
    pub package_id: String, pub package: String, pub kind: Vec<String>, pub target: String,
    pub features: Vec<String>, pub profile: Profile, pub filenames: Vec<PathBuf>,
    #[serde(deserialize_with="nullable")] pub executable: Option<PathBuf>,
    pub class: String, pub fresh: bool,
});
wire!(CargoTarget {
    pub kind: Vec<String>, pub crate_types: Vec<String>, pub name: String, pub src_path: PathBuf,
    pub edition: String, pub doc: bool, pub doctest: bool, pub test: bool,
});
wire!(CargoArtifact {
    pub reason: String, pub package_id: String, pub manifest_path: PathBuf, pub target: CargoTarget,
    pub profile: Profile, pub features: Vec<String>, pub filenames: Vec<PathBuf>,
    #[serde(deserialize_with="nullable")] pub executable: Option<PathBuf>, pub fresh: bool,
});
wire!(Artifact { pub package: String, pub kind: String, pub target: String, pub path: PathBuf, pub bytes: u64, pub sha256: String, pub profile: String });
wire!(CompilerCapture {
    pub classification: String, pub profile: String, pub target: String,
    #[serde(deserialize_with="nullable")] pub expected_manifest_sha256: Option<String>,
    pub build_inputs_before: BuildInputs, pub build_inputs_after: BuildInputs,
    pub units: Vec<Unit>, pub raw_compiler_artifact_json: Vec<String>,
    pub stable_artifacts: Vec<Artifact>, pub frozen_guard_verified: bool,
});
wire!(Compiler {
    pub normal: Pin, pub tests: Pin, pub normal_argv: Vec<String>, pub tests_argv: Vec<String>,
    pub frozen_manifest: Pin, pub embedded: EmbeddedInputs,
});
// A narrow supplement to unmodified shared compiler captures. Snapshots bracket
// the build; pinned rustc dep-info proves completeness for the selected images.
wire!(EmbeddedInputs { pub before: Vec<Pin>, pub after: Vec<Pin>, pub dep_info: Vec<Pin> });
wire!(FrozenManifest {
    pub format_version: u32, pub classification: String, pub profile: String, pub target: String,
    pub stable_directory: PathBuf, pub build_inputs: BuildInputs,
    pub units_identity: Vec<Value>, pub artifacts: Vec<Artifact>,
});
wire!(Resolution { pub name: String, pub path: PathBuf });
wire!(Workers {
    pub present: bool, #[serde(deserialize_with="nullable")] pub value: Option<String>,
    pub available: usize, pub effective: usize,
});
wire!(Runtime {
    pub metadata_shell: Pin, pub memory_shell: Pin, pub resolutions: Vec<Resolution>,
    pub environment: Vec<Environment>, pub configs: Vec<Config>, pub workers: Workers,
    pub cargo_jobs: u32, pub priority: String, pub offline: bool, pub locked: bool,
    pub skip_cache: bool, pub architecture: String,
});
wire!(Selection {
    pub isolated: Pin, pub list: Pin, pub harness_ref: String,
    pub named: u32, pub native_markers: u32, pub wrapper_markers: u32, pub aggregates: u32,
});
wire!(Inventory {
    pub schema: u32, pub classification: String, pub identity: Identity, pub deadline: Deadline,
    pub source: Source, pub compiler: Compiler, pub images: Vec<Image>,
    pub runtime: Runtime, pub selection: Selection,
});
wire!(Activation {
    pub schema: u32, pub classification: String, pub inventory: Pin,
    pub case: String, pub attempt: String, pub run_id: String, pub run: PathBuf,
    pub selector: String, pub implementation: String, pub execution_mode: String,
    pub deadline: Deadline, pub not_before_utc_ms: u64, pub not_after_utc_ms: u64,
    pub retry: String,
});
wire!(Launch {
    pub schema: u32, pub classification: String, pub inventory: Pin, pub activation: Pin, pub capture: Pin,
    pub case: String, pub attempt: String, pub run_id: String, pub run: PathBuf,
    pub owner_pid: u32, pub launcher_pid: u32, pub start_tick_ms: u64, pub start_utc_ms: u64,
    pub controller_tick_ms: u64, pub work_tick_ms: u64, pub outer_tick_ms: u64,
});
wire!(Capture { pub version: u32, pub attempt: String, pub owner_pid: u32, pub launcher_pid: u32, pub harness_path: PathBuf, pub whole_utc_ms: u64 });
wire!(IsolatedEntry { pub package: String, pub target: String, pub kind: String, pub name: String });
wire!(Isolated { pub format_version: u32, pub tests: Vec<IsolatedEntry> });
