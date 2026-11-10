//! Actual one-shot bootstrap stdout stays in zeroizing memory, never in logs.
use super::*;
use kilogram_bootstrap_contract::Ordered2BootstrapOutput;
use zeroize::Zeroizing;

fn bootstrap(base: &Path, index: usize, contexts: &[String; 2]) -> Result<Node> {
    let workspace = base.join(format!("bootstrap{index}"));
    let exe = repo().join("target/clean-start-runtime/kilogram-bootstrap.exe");
    ensure!(exe.is_file(), "fixed current bootstrap CLI required");
    let mut child = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x0800_4000);
    }
    let mut result = child
        .args(["create-ordered2", "--workspace-dir"])
        .arg(&workspace)
        .arg("--context")
        .args(contexts)
        .output()?;
    let secret_stdout = Zeroizing::new(std::mem::take(&mut result.stdout));
    ensure!(
        result.status.success(),
        "CS2 bootstrap child failed: {}; secret stdout omitted",
        result.status
    );
    let output = Ordered2BootstrapOutput::decode(&secret_stdout)
        .map_err(|_| anyhow::anyhow!("CS2 bootstrap result invalid; secret output omitted"))?;
    let info = output.public();
    ensure!(
        info.contexts == *contexts
            && info.workspace_dir.canonicalize()? == workspace.canonicalize()?,
        "explicit bootstrap context/workspace pins"
    );
    let schedule = base.join(format!("schedule{index}.json"));
    std::fs::write(&schedule, b"{}")?;
    let pins = serde_json::json!({"account":info.account_id.to_string(),"device":info.device_id.to_string(),"claim":info.claim});
    // Only public metadata escapes this scope. Output/phrase/stdout zeroize.
    Ok(Node {
        state: info.state_dir.clone(),
        profile: base.join(format!("profile{index}.json")),
        ipc: base.join(format!("ipc{index}.json")),
        locators: base.join(format!("locators{index}")),
        credential: info.credential_file.clone(),
        root: info.account_root_dir.clone(),
        schedule,
        pins,
        role: index as u8,
        child: None,
        starts: 0,
        memory_clock: std::sync::Mutex::new(Instant::now() - Duration::from_secs(5)),
    })
}
#[tokio::test]
async fn cs2_actual_bootstrap_cs1_import_and_runtime_open() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(150),async{
        let parent=repo().join(".tmp/clean-start-cs2/process");std::fs::create_dir_all(&parent)?;
        let base=tempfile::Builder::new().prefix("CS2-").tempdir_in(parent)?.keep();
        let contexts=[wire::hash(b"CS2-context0",text(&base).as_bytes()),wire::hash(b"CS2-context1",text(&base).as_bytes())].map(|id|hex(&id));
        let mut nodes=[bootstrap(&base,0,&contexts)?,bootstrap(&base,1,&contexts)?];
        if nodes[0].pins["device"].as_str()>nodes[1].pins["device"].as_str(){nodes.swap(0,1);}
        nodes[0].role=0;nodes[1].role=1;
        for (slot,c)in contexts.iter().enumerate(){let artifact=base.join(format!("context{slot}.bin"));fresh_consent::ceremony(&nodes,c,&artifact,slot==1)?;for node in &nodes{node.install(&artifact)?;}}
        // Profiles are no-clobber: create them only once after both imports.
        for node in &nodes{node.create_profile(&contexts)?;}
        let mut pair=Pair{nodes,contexts,base};
        for node in &mut pair.nodes{node.start().await?;let status=node.status().await?;ensure!(status["contexts"].as_array().is_some_and(|c|c.len()==2),"actual Runtime6 owner opens imported contexts");node.stop(false).await?;}
        println!("CS2_BOOTSTRAP_CS1_RUNTIME_PASS accounts=2 contexts=2 secret_stdout_logged=false actual_import_both=true runtime_open_both=true");
        Ok::<_,anyhow::Error>(())
    }).await.context("CS2 bounded integration watchdog")?
}
