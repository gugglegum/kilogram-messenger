use super::*;
use kilogram_cli::ordered_dialog::state::parse_id;
use kilogram_state::{StateDirectoryLock, indexed_dialog as db};
use positive::hit;
fn args(state: &Path, root: &Path, credential: &Path, contexts: &[String; 2]) -> Vec<String> {
    vec![
        "device-enroll".into(),
        "--profile-mode".into(),
        "ordered-dialog-v2".into(),
        "--state-dir".into(),
        text(state),
        "--account-dir".into(),
        text(root),
        "--ordered-context".into(),
        contexts[0].clone(),
        "--ordered-context".into(),
        contexts[1].clone(),
        "--certificate-file".into(),
        text(credential),
    ]
}
async fn enrollment_kill(
    base: &Path,
    root: &Path,
    contexts: &[String; 2],
    point: &str,
) -> Result<(PathBuf, PathBuf, serde_json::Value)> {
    let state = base.join(format!("{point}-state"));
    let credential = base.join(format!("{point}.credential"));
    let schedule = base.join(format!("{point}.json"));
    std::fs::write(
        &schedule,
        serde_json::to_vec(&serde_json::json!({"pause":point}))?,
    )?;
    let mut child = OwnedChild(
        command()
            .args(args(&state, root, &credential, contexts))
            .env("KILOGRAM_TEST_OD2_SCHEDULE", &schedule)
            .stdout(Stdio::from(std::fs::File::create(
                base.join(format!("{point}.stdout.log")),
            )?))
            .stderr(Stdio::from(std::fs::File::create(
                base.join(format!("{point}.stderr.log")),
            )?))
            .spawn()?,
    );
    let marker = hit(&schedule.with_extension("hit"), 20).await?;
    ensure!(
        marker["pid"] == child.id() && marker["point"] == point,
        "enrollment child barrier"
    );
    let pid = child.id();
    ensure!(
        child.try_wait()?.is_none(),
        "bootstrap barrier child already exited"
    );
    child.kill()?;
    child.wait()?;
    println!(
        "F04_parent_kill pid={pid} point={point} planned_public={}",
        marker["public"]
    );
    Ok((state, credential, marker["public"].clone()))
}
#[tokio::test]
async fn a3_s4_f_bootstrap_fence_committed_claim_explicit_resume() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(90),async{
        let parent=repo().join(".tmp/a3-s4/process");std::fs::create_dir_all(&parent)?;let base=tempfile::Builder::new().prefix("F-bootstrap-").tempdir_in(parent)?.keep();
        let root=base.join("root");let account=AccountRootState::create(&root)?;
        let contexts=[wire::hash(b"bootstrap C1",text(&base).as_bytes()),wire::hash(b"bootstrap C2",text(&base).as_bytes())].map(|id|hex(&id));
        let(state,credential,_)=enrollment_kill(&base,&root,&contexts,"enrollment_fenced").await?;
        let authority=account.authority_snapshot()?.encode()?;let output=command().args(args(&state,&root,&credential,&contexts)).output()?;
        ensure!(!output.status.success()&&String::from_utf8_lossy(&output.stderr).contains("CreationConflict"),"fence-only re-enrollment must fail closed");
        ensure!(account.authority_snapshot()?.encode()?==authority&&!credential.exists(),"fence-only retry changed Root/publication");
        let(state,credential,pins)=enrollment_kill(&base,&root,&contexts,"enrollment_committed").await?;ensure!(!credential.exists(),"public credential exported before barrier");
        let expected=db::ExpectedOwner{account:parse_id(pins["account"].as_str().context("account")?)?,claim:parse_id(pins["claim"].as_str().context("claim")?)?};
        let device=parse_id(pins["device"].as_str().context("device")?)?;
        {let mut repository=db::RuntimeRepository::open(StateDirectoryLock::acquire(&state)?,expected,db::Config::default())?;let p=repository.profile()?;ensure!(p.enrollment.device==device&&repository.generation()==1,"first RuntimeRoot6 claim/Device");}
        let resume=vec!["ordered".into(),"enrollment2-resume".into(),"--state-dir".into(),text(&state),"--expect-account".into(),hex(&expected.account),"--expect-claim".into(),hex(&expected.claim),"--expect-device".into(),hex(&device),"--certificate-file".into(),text(&credential)];
        let output=cli(resume.clone())?;ensure!(output.contains("enrollment-resumed"),"explicit same-key resume");let bytes=std::fs::read(&credential)?;cli(resume)?;ensure!(std::fs::read(&credential)?==bytes,"resume exact public bytes");
        let mut node=Node{state,profile:base.join("runtime.json"),ipc:base.join("runtime.ipc"),locators:base.join("locators"),credential,root,schedule:base.join("runtime.schedule.json"),pins,role:0,child:None,starts:0,memory_clock:std::sync::Mutex::new(Instant::now()-Duration::from_secs(5))};
        std::fs::write(&node.schedule,b"{}")?;node.create_profile(&contexts)?;node.start().await?;
        let status=node.status().await?;ensure!(status["contexts"].as_array().context("contexts")?.iter().all(|c|c["installed"]==false&&c["active"]==false)&&status["stats"]["packet_attempts"]==0,"enrolled runtime manufactured native/Active");
        let error=node.send(0,1).await.err().context("expected operation refusal")?;ensure!(format!("{error:#}").contains("NeedHandshake"),"queue before history Init");node.stop(false).await?;
        let mut reopened=db::RuntimeRepository::open(StateDirectoryLock::acquire(&node.state)?,expected,db::Config::default())?;
        ensure!(reopened.generation()==1&&reopened.profile()?.enrollment.device==device,"enrolled-only runtime changed committed claim");
        println!("S4_F04_BOOTSTRAP_PASS real_parent_kills=2 fence_only_closed=true first_root_runtime6=true committed_same_key_resume=true enrolled_ipc_no_native=true");Ok::<_,anyhow::Error>(())
    }).await.context("F04 bootstrap deadline")?
}
