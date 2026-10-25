//! Named fault cases retain actual application children and real durable operations.
use super::*;
use kilogram_cli::ordered_dialog::{runtime2::config::Profile2, state::parse_id};
use kilogram_state::{StateDirectoryLock, indexed_dialog as db};
use positive::{hit, selected};
pub(super) fn cold(node: &Node) -> Result<db::RuntimeRepository> {
    ensure!(
        node.child.is_none(),
        "cold inspection requires a stopped application"
    );
    let profile = Profile2::load(&node.profile)?;
    Ok(db::RuntimeRepository::open(
        StateDirectoryLock::acquire(&node.state)?,
        profile.owner()?,
        profile.db_config(),
    )?)
}
fn freeze(node: &Node) -> Result<serde_json::Value> {
    let mut r = cold(node)?;
    let s = r.snapshot(0)?;
    Ok(
        serde_json::json!({"generation":s.root_generation,"phase":s.head.phase,"active":s.head.active,"native_generation":s.head.native_generation,"native_hash":s.head.native_hash.map(|id|hex(&id)),"offer":s.head.offer.map(|id|hex(&id)),"grant":s.head.grant.map(|id|hex(&id)),"sid":s.head.sid,"q":s.head.mode_ref.map(|id|hex(&id)),"ready":s.head.ready.iter().map(|r|r.as_ref().map(|r|hex(&wire::hash(b"",r)))).collect::<Vec<_>>(),"counts":r.counts(0)?,"sequence":s.profile.next_author_sequence,"credits":r.credits()?}),
    )
}
fn protected(s: &serde_json::Value) -> Result<serde_json::Value> {
    Ok(serde_json::Value::Array(s["contexts"].as_array().context("contexts")?.iter().map(|c|serde_json::json!({"native":c["details"]["native_hash"],"generation":c["native_generation"],"counts":c["counts"],"active":c["active"]})).collect()))
}
async fn denied(pair: &Pair, node: usize, label: &str, needle: &str) -> Result<()> {
    let before = protected(&pair.nodes[node].status().await?)?;
    let error = pair.nodes[node]
        .send(0, 9000)
        .await
        .err()
        .with_context(|| format!("{label} unexpectedly accepted"))?;
    ensure!(
        format!("{error:#}").contains(needle),
        "{label} wrong refusal: {error:#}"
    );
    ensure!(
        protected(&pair.nodes[node].status().await?)? == before,
        "{label} protected logical state advanced"
    );
    println!("F01_case={label} refusal={error:#} protected_unchanged=true");
    Ok(())
}
async fn renew(pair: &Pair, nb: u64, expires: u64, label: &str) -> Result<Vec<u8>> {
    let path = pair.base.join(format!("{label}.consent"));
    cli(vec![
        "ordered".into(),
        "consent2-create".into(),
        "--context".into(),
        pair.contexts[0].clone(),
        "--credentials-file".into(),
        text(&pair.nodes[0].credential),
        text(&pair.nodes[1].credential),
        "--account-dir".into(),
        text(&pair.nodes[0].root),
        text(&pair.nodes[1].root),
        "--not-before".into(),
        nb.to_string(),
        "--valid-for-seconds".into(),
        (expires - nb).to_string(),
        "--output-file".into(),
        text(&path),
    ])?;
    let raw = std::fs::read(path)?;
    for node in &pair.nodes {
        let current = node.status().await?["contexts"][0]["details"]["current_consent"]
            .as_str()
            .context("current consent")?
            .to_owned();
        node.call(a([u(2), u(9), u(0), b(parse_id(&current)?), b(&raw), u(0)]))
            .await?;
    }
    Ok(raw)
}
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    ensure!(!destination.exists(), "fresh Root branch");
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "Root fixture symlink");
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
fn add_device(root: &AccountRootState) -> Result<()> {
    let device = kilogram_identity::DeviceIdentity::generate()?;
    let encryption = kilogram_identity::DeviceEncryptionIdentity::generate()?;
    root.issue_device_certificate(
        device.device_id(),
        encryption.public_key(),
        &kilogram_identity::DeviceCapability::MESSAGING,
    )?;
    Ok(())
}
async fn import_authority(node: &Node, raw: &[u8]) -> Result<()> {
    node.call(a([u(2), u(8), b(raw)])).await?;
    Ok(())
}
fn pause(node: &Node, name: &str) -> Result<()> {
    let marker = node.schedule.with_extension("hit");
    if marker.exists() {
        std::fs::remove_file(marker)?;
    }
    node.schedule(serde_json::json!({"slot":0,"pause":name}))
}
async fn killed_at(pair: &mut Pair, node: usize, name: &str) -> Result<serde_json::Value> {
    let marker = hit(&pair.nodes[node].schedule.with_extension("hit"), 60).await?;
    ensure!(
        marker["point"] == name
            && marker["pid"] == pair.nodes[node].child.as_ref().context("child")?.id(),
        "exact child barrier"
    );
    ensure!(
        tokio::time::timeout(
            Duration::from_millis(120),
            pair.nodes[node].call(a([u(2), u(1)]))
        )
        .await
        .is_err(),
        "owner was not actually paused at durable barrier"
    );
    pair.nodes[node].stop(true).await?;
    let state = freeze(&pair.nodes[node])?;
    println!("F_parent_kill point={name} node={node} marker={marker} recovered={state}");
    Ok(state)
}
fn only_first_context(pair: &Pair) -> Result<()> {
    for node in &pair.nodes {
        let mut p: serde_json::Value = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
        p["enabled"] = serde_json::json!([0]);
        std::fs::write(&node.profile, serde_json::to_vec_pretty(&p)?)?;
    }
    Ok(())
}

#[tokio::test]
async fn a3_s4_f_parent_kills_fresh_native_queue_ingress_receipt() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(400),async{
        let mut pair=Pair::prepared("F-kill-")?;only_first_context(&pair)?;
        pause(&pair.nodes[0],"PrepareOffer.committed")?;pair.nodes[0].start().await?;pair.nodes[1].start().await?;
        pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let offered=killed_at(&mut pair,0,"PrepareOffer.committed").await?;ensure!(offered["phase"]==1&&offered["counts"]["sources"]==0,"OfferedA");
        pause(&pair.nodes[1],"ReserveGrant.committed")?;pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;
        pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;
        let granted=killed_at(&mut pair,1,"ReserveGrant.committed").await?;ensure!(granted["phase"]==2&&granted["offer"]==offered["offer"],"GrantedB same Offer");
        pause(&pair.nodes[0],"Genesis.committed")?;pair.nodes[1].schedule(serde_json::json!({}))?;pair.nodes[1].start().await?;
        pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let init=killed_at(&mut pair,0,"Genesis.committed").await?;ensure!(init["phase"]==3&&init["counts"]["sources"]==1&&init["grant"]==granted["grant"],"durable A0");
        pause(&pair.nodes[1],"Genesis.committed")?;pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;
        pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;
        let confirm=killed_at(&mut pair,1,"Genesis.committed").await?;ensure!(confirm["counts"]["sources"]==2&&confirm["sid"]==init["sid"]&&confirm["q"]==init["q"],"atomic B A0/Confirm");
        pause(&pair.nodes[0],"Ready.committed")?;pair.nodes[1].schedule(serde_json::json!({}))?;pair.nodes[1].start().await?;
        pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let own_ready=killed_at(&mut pair,0,"Ready.committed").await?;ensure!(own_ready["active"]==false&&own_ready["ready"][0].is_string()&&own_ready["ready"][1].is_null(),"own Ready before peer Ready");
        pair.nodes[1].stop(true).await?;pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;pair.nodes[0].send(0,1).await?;
        let waiting=pair.nodes[0].status().await?;ensure!(waiting["contexts"][0]["active"]==false&&waiting["contexts"][0]["counts"]["queued"]==1&&waiting["contexts"][0]["counts"]["sources"]==2,"queued waits durable Ready2");
        pause(&pair.nodes[0],"Ready.committed")?;pair.nodes[1].start().await?;pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let active=killed_at(&mut pair,0,"Ready.committed").await?;ensure!(active["active"]==true&&active["counts"]["sources"]==2&&active["counts"]["queued"]==1,"Active2 before first Text publication");
        pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.converged(0,3).await?;
        pause(&pair.nodes[0],"Queue.committed")?;
        let queue_hit=pair.nodes[0].schedule.with_extension("hit");
        tokio::select!{result=pair.nodes[0].send(0,2)=>{anyhow::bail!("queue replied before committed barrier: {result:?}");},result=hit(&queue_hit,30)=>{result?;}}
        let queued=killed_at(&mut pair,0,"Queue.committed").await?;ensure!(queued["counts"]["queued"]==1&&queued["counts"]["sources"]==3,"queue before IPC reply");
        pause(&pair.nodes[0],"Materialize.committed")?;pair.nodes[0].launch()?;
        let materialized=killed_at(&mut pair,0,"Materialize.committed").await?;ensure!(materialized["counts"]["queued"]==0&&materialized["counts"]["sources"]==4&&materialized["counts"]["debt"]==1,"materialized before socket publication");
        let(request2,_)=message(0,0,2);let accepted=cold(&pair.nodes[0])?.request(0,request2)?.context("accepted request2")?.0;
        ensure!(accepted.event.is_some(),"accepted request materialized");
        pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.nodes[0].send(0,2).await?;pair.converged(0,4).await?;
        pause(&pair.nodes[1],"AcceptPending.committed")?;pair.nodes[0].send(0,3).await?;
        let pending=killed_at(&mut pair,1,"AcceptPending.committed").await?;ensure!(pending["counts"]["pending"]==1&&pending["counts"]["sources"]==4,"Pending before Complete");
        let pending_hash={let p=cold(&pair.nodes[1])?.pending(0,3)?.context("committed pending")?;hex(&wire::hash(b"",&p.body))};
        pause(&pair.nodes[1],"Drain.committed")?;pair.nodes[1].start().await?;pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let readable=killed_at(&mut pair,1,"Drain.committed").await?;ensure!(readable["counts"]["sources"]==5&&readable["counts"]["pending"]==0,"readable before ACK publication");
        let receipt={let mut repo=cold(&pair.nodes[1])?;ensure!(hex(&wire::hash(b"",&repo.source(0,0,3)?.context("readable source")?.raw))==pending_hash,"pending/source exact bytes");repo.receipt(0,0,3)?.context("durable receipt")?};
        pause(&pair.nodes[0],"Acknowledge.committed")?;pair.nodes[1].schedule(serde_json::json!({}))?;pair.nodes[1].start().await?;pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        let acknowledged=killed_at(&mut pair,0,"Acknowledge.committed").await?;ensure!(acknowledged["counts"]["debt"]==0,"receipt/debt before Delivered IPC reply");
        let(request3,_)=message(0,0,3);{let mut repo=cold(&pair.nodes[0])?;ensure!(repo.request_delivery(0,request3)?==Some(2)&&repo.receipt(0,0,3)?.as_ref()==Some(&receipt),"same receipt and durable Delivered");}
        pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].start().await?;pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.converged(0,5).await?;
        for node in 0..2{pair.history(node,0,3).await?;}for node in &mut pair.nodes{node.stop(false).await?;}
        let mut accounts=0;let mut sessions=0;for node in &pair.nodes{for start in 1..=node.starts{let log=std::fs::read_to_string(node.profile.with_extension(format!("{start}.stderr.log")))?;accounts+=log.lines().filter(|l|l.starts_with("od2_account_constructor context=0 ")).count();sessions+=log.lines().filter(|l|l.starts_with("od2_session_constructor context=0 ")).count();}}
        ensure!(accounts==2&&sessions==2,"restart recreated Account/Session: {accounts}/{sessions}");
        println!("S4_F_KILLS_PASS fault_window_kills=11 offline_stop_kill=1 source_bytes_exact=true constructors_accounts={accounts} sessions={sessions} delivered_after_receipt_only=true");Ok::<_,anyhow::Error>(())
    }).await.context("F real kill sequence deadline")?
}

#[tokio::test]
async fn a3_s4_f_policy_renewal_post_staging_real_close() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(400),async{
        let mut pair=Pair::new("F-policy-").await?;pair.nodes[0].send(0,1).await?;pair.converged(0,3).await?;
        let initial=selected(&pair.nodes[0].status().await?)?;
        pair.nodes[0].schedule(serde_json::json!({"clock_failure":true}))?;denied(&pair,0,"clock_failure","ClockBlocked").await?;
        pair.history(0,0,1).await?;
        pair.nodes[0].schedule(serde_json::json!({}))?;
        let wall=pair.nodes[0].status().await?["contexts"][0]["details"]["wall_high_water"].as_u64().context("clock high-water")?;
        pair.nodes[0].schedule(serde_json::json!({"clock":wall-1}))?;denied(&pair,0,"clock_backward","ClockBlocked").await?;
        pair.nodes[0].schedule(serde_json::json!({"authority_read_failure":true}))?;denied(&pair,0,"authority_read_failure","AuthorityReadFailure").await?;
        pair.nodes[0].send(0,1).await?;let(request,_)=message(0,0,1);
        let error=pair.nodes[0].call(a([u(2),u(2),u(0),b(request),b("different exact body")])).await.err().context("expected operation refusal")?;ensure!(format!("{error:#}").contains("RequestIdConflict"),"exact retry conflict");
        println!("F09_exact_retry_before_current_permission=true conflict_explicit=true");pair.nodes[0].schedule(serde_json::json!({}))?;
        let wall=kilogram_cli::ordered_dialog::runtime2::now()?;
        renew(&pair,wall+120,wall+600,"not-before").await?;denied(&pair,0,"consent_not_before","ConsentExpired/NotYetValid").await?;
        renew(&pair,wall-600,wall-1,"expired").await?;denied(&pair,0,"consent_expiry","ConsentExpired/NotYetValid").await?;
        let valid=renew(&pair,wall-10,wall+3600,"renewed").await?;
        ensure!(selected(&pair.nodes[0].status().await?)?==initial,"F02 current renewal changed selected proof/Q");
        let fork1=pair.base.join("authority-branch1");let fork2=pair.base.join("authority-branch2");copy_tree(&pair.nodes[0].root,&fork1)?;copy_tree(&pair.nodes[0].root,&fork2)?;
        let root1=AccountRootState::load(&fork1)?;let root2=AccountRootState::load(&fork2)?;
        root1.revoke_device(kilogram_identity::DeviceIdentity::generate()?.device_id())?;
        root2.revoke_device(kilogram_identity::DeviceIdentity::generate()?.device_id())?;
        let first=root1.authority_snapshot()?.encode()?;let conflict=root2.authority_snapshot()?.encode()?;
        let original=AccountRootState::load(&pair.nodes[0].root)?.authority_snapshot()?.encode()?;
        import_authority(&pair.nodes[0],&first).await?;import_authority(&pair.nodes[1],&first).await?;
        for(label,raw)in [("lower_revision",original),("same_revision_changed_bytes",conflict)]{
            let before=protected(&pair.nodes[0].status().await?)?;let error=import_authority(&pair.nodes[0],&raw).await.err().context("expected operation refusal")?;ensure!(format!("{error:#}").contains("rollback/conflict"),"authority refusal {error:#}");ensure!(before==protected(&pair.nodes[0].status().await?)?,"authority rejection advanced");println!("F01_case={label} valid_Root_signature=true refused=true");
        }
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"pause":"Materialize.staged"}))?;pair.nodes[0].send(0,2).await?;
        let staged=hit(&pair.nodes[0].schedule.with_extension("hit"),45).await?;let expires=wire::ParsedConsent2::parse(&valid)?.interval().1;
        pair.nodes[0].schedule(serde_json::json!({"clock":expires}))?;
        let rejected=pair.state(0,Duration::from_secs(15),|s|s["stats"]["aborted_packet_calls"].as_u64().unwrap_or(0)>=1,"post-staging clock abort").await?;
        ensure!(rejected["contexts"][0]["details"]["native_hash"]==staged["public"]["native_hash"]&&rejected["contexts"][0]["counts"]["queued"]==1&&rejected["stats"]["reopens"]==0,"clock abort changed accepted Native/queue or poisoned");
        pair.nodes[0].schedule(serde_json::json!({}))?;pair.converged(0,4).await?;println!("F01_case=post_staging_expiry clean_abort=true trial_calls_separate=true reusable=true");
        std::fs::remove_file(pair.nodes[0].schedule.with_extension("hit"))?;
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"pause":"Drain.staged"}))?;pair.nodes[1].send(0,1).await?;
        let staged=hit(&pair.nodes[0].schedule.with_extension("hit"),60).await?;let aborted=rejected["stats"]["aborted_packet_calls"].as_u64().context("aborted count")?;
        pair.nodes[1].stop(false).await?;tokio::time::sleep(Duration::from_millis(200)).await;pair.nodes[0].schedule(serde_json::json!({}))?;
        let rejected=pair.state(0,Duration::from_secs(15),|s|s["stats"]["aborted_packet_calls"].as_u64().unwrap_or(0)>aborted,"post-staging actual close abort").await?;
        ensure!(rejected["contexts"][0]["details"]["native_hash"]==staged["public"]["native_hash"]&&rejected["contexts"][0]["counts"]["pending"]==1&&rejected["stats"]["reopens"]==0,"actual close abort changed accepted state");
        pair.nodes[0].send(0,3).await?;pair.state(0,Duration::from_secs(30),|s|s["contexts"][0]["counts"]["queued"]==0,"valid local encryption after clean abort").await?;
        pair.nodes[1].start().await?;pair.import().await?;pair.converged(0,6).await?;println!("F01_case=post_staging_actual_connection_close clean_abort=true reusable=true");
        let device=kilogram_identity::DeviceId::from_bytes(parse_id(pair.nodes[0].pins["device"].as_str().context("device")?)?);root1.revoke_device(device)?;
        let revoked=root1.authority_snapshot()?.encode()?;import_authority(&pair.nodes[0],&revoked).await?;import_authority(&pair.nodes[1],&revoked).await?;
        denied(&pair,0,"own_revocation","revoked").await?;denied(&pair,1,"peer_revocation","revoked").await?;pair.history(0,0,4).await?;pair.history(1,0,4).await?;
        pair.nodes[0].send(0,1).await?;
        add_device(&root2)?;add_device(&root2)?;let forgotten=root2.authority_snapshot()?.encode()?;
        let error=import_authority(&pair.nodes[0],&forgotten).await.err().context("expected operation refusal")?;ensure!(format!("{error:#}").contains("known revocation"),"forgotten revoke: {error:#}");
        for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_F_POLICY_PASS current_vs_historical=true monotonic_authority=true local_read_after_revoke=true clean_gates=2");Ok::<_,anyhow::Error>(())
    }).await.context("F policy process deadline")?
}

fn edit_profile(node: &Node, edit: impl FnOnce(&mut serde_json::Value)) -> Result<()> {
    ensure!(
        node.child.is_none(),
        "configuration changes require a stopped runtime"
    );
    let mut p = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
    edit(&mut p);
    std::fs::write(&node.profile, serde_json::to_vec_pretty(&p)?)?;
    Ok(())
}
async fn failed_start(node: &mut Node, needle: &str) -> Result<()> {
    let log = node.launch()?;
    let mut child = node.child.take().context("startup child")?;
    let start = Instant::now();
    loop {
        if let Some(exit) = child.try_wait()? {
            let output = std::fs::read_to_string(log)?;
            ensure!(
                !exit.success() && output.contains(needle),
                "wrong startup refusal {exit}: {output}"
            );
            println!(
                "F_expected_start_refusal pid={} reason={needle}",
                child.id()
            );
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(20) {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("expected startup refusal timed out");
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
#[tokio::test]
async fn a3_s4_f_backend_io_ambiguity_quota_capacity_reopen() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(240),async{
        let mut pair=Pair::new("F-io-").await?;pair.nodes[0].send(0,1).await?;pair.converged(0,3).await?;
        let mut reopens=0;
        for (n,point,token) in [(2,"before_write","queue-prewrite"),(3,"after_sync","queue-sync")]{
            pair.nodes[0].schedule(serde_json::json!({"slot":0,"io_operation":"Queue","io_fault":point,"io_token":token}))?;
            let result=pair.nodes[0].send(0,n).await;println!("F07_actual_backend_fault point={point} result={result:?}");
            let state=pair.state(0,Duration::from_secs(30),|s|s["stats"]["reopens"].as_u64().unwrap_or(0)>reopens,"same-lock IO reopen").await?;reopens=state["stats"]["reopens"].as_u64().context("reopens")?;
            ensure!(state["stats"]["ambiguous_packet_calls"]==0,"unresolved packet outcome after recovery");
            pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].send(0,n).await?;pair.converged(0,n+2).await?;
        }
        for (n,point,token) in [(4,"before_write","native-prewrite"),(5,"after_sync","native-sync")]{
            let before=pair.nodes[0].status().await?;
            pair.nodes[0].schedule(serde_json::json!({"slot":0,"io_operation":"Materialize","io_fault":point,"io_token":token}))?;pair.nodes[0].send(0,n).await?;
            let state=pair.state(0,Duration::from_secs(30),|s|s["stats"]["reopens"].as_u64().unwrap_or(0)>reopens,"native materialization IO reopen").await?;reopens=state["stats"]["reopens"].as_u64().context("reopens")?;
            pair.nodes[0].schedule(serde_json::json!({}))?;pair.nodes[0].send(0,n).await?;pair.converged(0,n+2).await?;
            let after=pair.nodes[0].status().await?;let delta=after["stats"]["packet_attempts"].as_u64().context("trials")?-before["stats"]["packet_attempts"].as_u64().context("trials")?;
            ensure!(after["stats"]["ambiguous_packet_calls"]==0&&after["stats"]["committed_encrypt"].as_u64().context("committed")?==before["stats"]["committed_encrypt"].as_u64().context("committed")?+1,"native IO outcome accounting");
            ensure!(delta==if point=="after_sync"{1}else{2},"actual committed/aborted Native trial replay: {point} {delta}");println!("F07_native_fault operation=Materialize point={point} actual_attempt_delta={delta} committed_delta=1 unresolved=0");
        }
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"io_operation":"AckProgress","io_fault":"before_write","io_token":"ack-cursor"}))?;
        pair.nodes[0].send(0,6).await?;
        pair.state(1,Duration::from_secs(45),|s|s["stats"]["reopens"].as_u64().unwrap_or(0)>0,"ACK scheduling cursor IO reopen").await?;
        pair.nodes[1].schedule(serde_json::json!({}))?;pair.converged(0,8).await?;
        pair.nodes[0].stop(false).await?;
        let before=freeze(&pair.nodes[0])?;let limits={let repo=cold(&pair.nodes[0])?;let credits=repo.credits()?;std::array::from_fn::<_,2,_>(|c|credits.used[c]+credits.reserved[c]+credits.repair_free[c]+credits.grant_unissued[c]+if credits.head_in_use[c]{0}else{db::TEMP_CREDIT})};
        edit_profile(&pair.nodes[0],|p|p["physical_quota"]=serde_json::json!(db::HEADROOM))?;
        failed_start(&mut pair.nodes[0],"ConfiguredCapacity").await?;
        edit_profile(&pair.nodes[0],|p|{p["physical_quota"]=serde_json::json!(db::Config::default().physical_quota);p["logical_quota"]=serde_json::json!(limits);})?;
        ensure!(freeze(&pair.nodes[0])?==before,"capacity refusal changed logical records");
        pair.nodes[0].start().await?;pair.nodes[1].import(0,&pair.nodes[0],&pair.contexts).await?;pair.nodes[1].import(1,&pair.nodes[0],&pair.contexts).await?;
        let original=protected(&pair.nodes[0].status().await?)?;pair.nodes[0].send(0,1).await?;
        let error=pair.nodes[0].send(0,7).await.err().context("expected operation refusal")?;ensure!(format!("{error:#}").contains("quota"),"configured logical quota gate {error:#}");
        ensure!(original==protected(&pair.nodes[0].status().await?)?,"quota refusal advanced accepted state");
        pair.nodes[0].stop(false).await?;edit_profile(&pair.nodes[0],|p|p["logical_quota"]=serde_json::json!(limits.map(|q|q+2*db::FRAME_CREDIT)))?;
        pair.nodes[0].start().await?;pair.import().await?;pair.nodes[0].send(0,7).await?;pair.converged(0,9).await?;
        for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_F_IO_PASS actual_backend_faults=5 native_ambiguity_resolved_without_replay=true configured_capacity_refusal=true logical_quota_refusal=true increase_reopen_retry=true hardware_full_volume_claim=false");Ok::<_,anyhow::Error>(())
    }).await.context("F IO/capacity deadline")?
}
fn predecessor(
    mode: &str,
    path: &Path,
    owner: db::ExpectedOwner,
    config: db::Config,
) -> Result<String> {
    let mut cmd = Command::new(repo().join(
        "target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe",
    ));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_4000);
    }
    let output = cmd
        .args([
            mode,
            &text(path),
            &hex(&owner.account),
            &hex(&owner.claim),
            &config.logical_quota[0].to_string(),
            &config.logical_quota[1].to_string(),
            &config.physical_quota.to_string(),
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "predecessor {mode}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}
#[tokio::test]
async fn a3_s4_f09_actual_trial_operational_failure_rollback() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(120),async{
        let mut pair=Pair::new("F-operational-").await?;pair.nodes[0].send(0,1).await?;pair.converged(0,3).await?;
        let before=protected(&pair.nodes[1].status().await?)?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"suspend_drain":true}))?;
        for n in 2..=5{pair.nodes[0].send(0,n).await?;}
        pair.state(1,Duration::from_secs(30),|s|s["contexts"][0]["counts"]["pending"]==4,"four actual pending packets before batch trial").await?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"operational_after_trial_number":4}))?;
        let state=pair.state(1,Duration::from_secs(30),|s|s["contexts"][0]["counts"]["pending"]==4&&s["stats"]["aborted_packet_calls"].as_u64().unwrap_or(0)>=4,"actual trial OperationalFailure").await?;
        let c=&state["contexts"][0];
        ensure!(c["details"]["native_hash"]==before[0]["native"]&&c["counts"]["sources"]==3&&c["blocked"].is_null(),"OperationalFailure mutated accepted native/prefix or blocked C");
        for n in 1..=2{pair.nodes[0].send(1,n).await?;pair.nodes[1].send(1,n).await?;}pair.converged(1,6).await?;
        pair.history(1,0,1).await?;
        pair.nodes[1].schedule(serde_json::json!({}))?;pair.converged(0,7).await?;pair.history(1,0,5).await?;
        for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_F09_OPERATIONAL_PASS actual_native_trial=true injected_failure_after_trial=true batch_size=4 failure_after_actual_trial=4 accepted_checkpoint_rollback=true old_prefix_readable=true no_receipt_until_retry_commit=true C2_progress=true");Ok::<_,anyhow::Error>(())
    }).await.context("F09 operational deadline")?
}
#[tokio::test]
async fn a3_s4_f_exact_s3_predecessor_fence_positive_controls() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(90),async{
        let mut pair=Pair::new("F-predecessor-").await?;pair.nodes[0].send(0,1).await?;pair.nodes[1].send(0,1).await?;pair.converged(0,4).await?;
        for node in &mut pair.nodes{node.stop(false).await?;}
        let positive=pair.base.join("component-positive-s3");let expected=db::ExpectedOwner{account:[1;32],claim:[2;32]};
        let result=predecessor("positive",&positive,expected,db::Config::default())?;ensure!(result.contains("S3_PREDECESSOR_POSITIVE"),"positive control");println!("{result}");
        let mut current=db::IndexedDialogVault::open(StateDirectoryLock::acquire(&positive)?,expected,db::Config::default())?;
        ensure!(current.get(db::Key::hash(0,db::Namespace::Control,[12;32])?)?.as_deref()==Some(b"legitimate component".as_slice()),"current component loader positive control");drop(current);
        let fresh_component=pair.base.join("component-positive-current");
        let mut current=db::IndexedDialogVault::create(StateDirectoryLock::acquire(&fresh_component)?,db::CreationClaim{account:expected.account,claim:expected.claim,signing_secret:zeroize::Zeroizing::new([3;32]),encryption_secret:zeroize::Zeroizing::new([4;32]),contexts:[[5;32],[6;32]]},db::Config::default())?;
        current.commit(current.generation(),[31;32],vec![db::Mutation::Put{key:db::Key::hash(0,db::Namespace::Control,[12;32])?,value:b"current legitimate component".to_vec()}],vec![])?;drop(current);
        let mut current=db::IndexedDialogVault::open(StateDirectoryLock::acquire(&fresh_component)?,expected,db::Config::default())?;ensure!(current.get(db::Key::hash(0,db::Namespace::Control,[12;32])?)?.as_deref()==Some(b"current legitimate component".as_slice()),"current-created ComponentRoot5 positive control");drop(current);
        for node in &pair.nodes{let before=freeze(node)?;let p=Profile2::load(&node.profile)?;
            ensure!(matches!(db::IndexedDialogVault::open(StateDirectoryLock::acquire(&node.state)?,p.owner()?,p.db_config()),Err(db::Error::UnsupportedRuntimeProfile)),"current component runtime fence");
            let result=predecessor("probe",&node.state,p.owner()?,p.db_config())?;ensure!(result.contains("S3_PREDECESSOR_REFUSED"),"actual predecessor gate");
            ensure!(freeze(node)?==before,"old open changed protected RuntimeRoot6 logical records");println!("{result}");
        }
        println!("S4_F10_PASS predecessor=7f82036 correct_owner_config=true positive_component5=true runtime6_refused_before_writer=true logical_state_unchanged=true");Ok::<_,anyhow::Error>(())
    }).await.context("F10 predecessor deadline")?
}
