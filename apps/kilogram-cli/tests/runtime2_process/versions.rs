use super::*;
use kilogram_cli::ordered_dialog::runtime2::config::Profile2;
use kilogram_state::indexed_dialog as db;
fn rejected(mut command: Command, args: &[String]) -> Result<String> {
    let output = command.args(args).output()?;
    ensure!(
        !output.status.success(),
        "version/bypass command unexpectedly succeeded {args:?}"
    );
    let error = String::from_utf8(output.stderr)?;
    println!("F08_refusal args={args:?} reason={error}");
    Ok(error)
}
fn old_command() -> Command {
    let old = repo().join("target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe");
    let mut cmd = Command::new(old);
    cmd.current_dir(repo());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_4000);
    }
    cmd
}
#[tokio::test]
async fn a3_s4_f08_baseline_l1_small_history_and_version_bypass_gates() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(160),async{
        let mut pair=Pair::prepared("F-versions-")?;
        let baseline_run=pair.base.join("fresh-L1-d9b");let log=pair.base.join("baseline-L1.controller.log");
        let mut powershell=Command::new("pwsh.exe");#[cfg(windows)]{use std::os::windows::process::CommandExt;powershell.creation_flags(0x0800_4000);}
        let mut helper=OwnedChild(powershell.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(repo().join("ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence/baseline-l1-control.ps1"))
            .arg("-RunDirectory").arg(&baseline_run)

            .stdout(Stdio::from(std::fs::File::create(&log)?)).stderr(Stdio::from(std::fs::File::create(pair.base.join("baseline-L1.controller.stderr.log"))?)).spawn()?);
        let start=Instant::now();loop{if let Some(exit)=helper.try_wait()?{ensure!(exit.success(),"baseline L1 helper failed: {}",std::fs::read_to_string(&log)?);break;}ensure!(start.elapsed()<Duration::from_secs(100),"baseline L1 deadline");tokio::time::sleep(Duration::from_millis(50)).await;}
        let result:serde_json::Value=serde_json::from_slice(&std::fs::read(baseline_run.join("result.json"))?)?;ensure!(result["status"]=="PASS"&&result["text_count"]==4,"fresh L1 positive control");println!("F08_actual_baseline_L1={result}");
        let before=faults::cold(&pair.nodes[0])?.generation();
        for cmd in [command(),old_command()]{let error=rejected(cmd,&["ordered".into(),"inspect".into(),"--state-dir".into(),text(&pair.nodes[0].state)])?;ensure!(error.contains("schema")||error.contains("Unsupported")||error.contains("unsupported"),"runtime2-as-V1 refused for wrong reason");}
        rejected(command(),&["identity".into(),"--state-dir".into(),text(&pair.nodes[0].state)])?;
        let original=std::fs::read(&pair.nodes[0].profile)?;let mut bad:serde_json::Value=serde_json::from_slice(&original)?;bad["native_generation"]=serde_json::json!(999);
        let edited=pair.base.join("edited-unknown.json");std::fs::write(&edited,serde_json::to_vec(&bad)?)?;
        let error=rejected(command(),&["runtime-from-profile".into(),"--profile-file".into(),text(&edited)])?;ensure!(error.contains("unknown field"),"unknown JSON not refused at typed profile admission");
        let state_l1=baseline_run.join("one.state");let p=Profile2::load(&pair.nodes[0].profile)?;
        ensure!(matches!(db::RuntimeRepository::open(kilogram_state::StateDirectoryLock::acquire(&state_l1)?,p.owner()?,p.db_config()),Err(db::Error::ExistingStateRequiresMigration)),"V4-as-V2 safe refusal");
        let component=pair.base.join("legitimate-component5");let owner=p.owner()?;
        let store=db::IndexedDialogVault::create(kilogram_state::StateDirectoryLock::acquire(&component)?,db::CreationClaim{account:owner.account,claim:owner.claim,signing_secret:zeroize::Zeroizing::new([3;32]),encryption_secret:zeroize::Zeroizing::new([4;32]),contexts:[[5;32],[6;32]]},p.db_config())?;drop(store);
        ensure!(matches!(db::RuntimeRepository::open(kilogram_state::StateDirectoryLock::acquire(&component)?,owner,p.db_config()),Err(db::Error::UnsupportedRuntimeProfile)),"ComponentRoot5 adoption into runtime2 must be explicit unsupported");
        bad=serde_json::from_slice(&original)?;bad["device"]=serde_json::json!("01".repeat(32));std::fs::write(&edited,serde_json::to_vec(&bad)?)?;
        let error=rejected(command(),&["runtime-from-profile".into(),"--profile-file".into(),text(&edited)])?;ensure!(error.contains("profile Device/context pins"),"profile pins checked after native restore");
        ensure!(faults::cold(&pair.nodes[0])?.generation()==before,"cold version admission changed protected root");
        pair.nodes[0].start().await?;pair.nodes[1].start().await?;pair.import().await?;pair.wait_active().await?;
        ensure!(matches!(kilogram_runtime_ipc::call(&pair.nodes[0].ipc,RuntimeIpcCommand::WaitForChange{after_revision:0,timeout_milliseconds:1}).await?,RuntimeIpcResponse::Error{..}),"ordinary live writer/WaitForChange not rejected before repository");
        let ordinary=RuntimeIpcCommand::AddContact{conversation:"forbidden".into(),expected_peer_account_id:kilogram_identity::AccountId::from_bytes([9;32]),descriptor_file:pair.base.join("must-not-be-opened.json")};
        ensure!(matches!(kilogram_runtime_ipc::call(&pair.nodes[0].ipc,ordinary).await?,RuntimeIpcResponse::Error{message}if message.contains("UnsupportedProfile")),"ordinary live mutator crossed mode admission");
        let endpoint=iroh::Endpoint::builder(iroh::endpoint::presets::Minimal).clear_address_lookup().clear_ip_transports().clear_relay_transports().bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>()?)?.alpns(vec![kilogram_protocol::ordered_dialog_v1::ALPN.to_vec()]).bind().await?;
        let raw=std::fs::read(pair.nodes[1].locators.join(format!("{}.locator",pair.contexts[0])))?;let loc=wire::ParsedRuntimeLocator2::parse(&raw)?;
        let remote=kilogram_transport_iroh::ordered2::RouteConfig2{policy:kilogram_transport_iroh::RoutePolicy::DirectOnly,bind:Some("127.0.0.1:0".parse()?),relay:None}.remote(&loc)?;
        ensure!(tokio::time::timeout(Duration::from_secs(5),endpoint.connect(remote,kilogram_protocol::ordered_dialog_v1::ALPN)).await?.is_err(),"old wire peer ALPN accepted");endpoint.close().await;
        for node in &mut pair.nodes{node.stop(false).await?;}
        // Resume the actual V4 history through the current explicit V1 runtime.
        let profile=baseline_run.join("one.profile");let mut runtime=OwnedChild(command().args(["runtime-from-profile","--profile-file"]).arg(&profile).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(pair.base.join("current-L1.stderr.log"))?)).spawn()?);
        let ipc=baseline_run.join("one.ipc");let start=Instant::now();let status=loop{if let Some(exit)=runtime.try_wait()?{anyhow::bail!("current V1 runtime exited {exit}");}let output=command().args(["ordered","status","--ipc-file"]).arg(&ipc).output()?;if output.status.success(){break serde_json::from_slice::<serde_json::Value>(&output.stdout)?;}ensure!(start.elapsed()<Duration::from_secs(20),"current L1 startup");tokio::time::sleep(Duration::from_millis(50)).await;};
        ensure!(status["profile_mode"]=="ordered-dialog-v1","current V1 resumed wrong mode");
        let context=result["context"].as_str().context("L1 context")?;let output=cli(vec!["ordered".into(),"history".into(),"--ipc-file".into(),text(&ipc),"--context".into(),context.into(),"--limit".into(),"8".into()])?;
        let history:serde_json::Value=serde_json::from_str(&output)?;ensure!(history["items"].as_array().context("V1 items")?.len()==4,"current V1 old history inaccessible");
        kilogram_runtime_ipc::call(&ipc,RuntimeIpcCommand::Shutdown).await?;runtime.wait()?;
        println!("S4_F08_VERSION_PASS fresh_d9b_L1_text=4 V4_as_V2_refused=true runtime2_as_current_and_old_V1_refused=true ordinary_cold_live_refused=true edited_unknown_JSON_refused=true profile_pins_pre_native=true old_ALPN_refused=true current_V1_old_history_readable=true marker_component_existing_origin_covered_by_named_state_actor_cases=true");Ok::<_,anyhow::Error>(())
    }).await.context("F08 version/bypass deadline")?
}
