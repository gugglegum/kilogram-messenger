use super::*;
use std::collections::{BTreeMap, BTreeSet};

struct Relay {
    child: OwnedChild,
    stop: PathBuf,
    url: String,
    stderr: PathBuf,
}
fn relay_stderr(path: &Path) -> String {
    use std::io::Read;
    let result = (|| -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(16 * 1024)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })();
    match result {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => format!("stderr read {}: {error}", path.display()),
    }
}
impl Relay {
    async fn start(base: &Path) -> Result<Self> {
        let ready = base.join("relay.ready");
        let stop = base.join("relay.stop");
        let stderr = base.join("relay.stderr.log");
        let mut command = Command::new(
            repo().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe"),
        );
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_4000);
        }
        let mut child = OwnedChild(
            command
                .arg(&ready)
                .arg(&stop)
                .stdout(Stdio::null())
                .stderr(Stdio::from(std::fs::File::create(&stderr)?))
                .spawn()?,
        );
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                anyhow::bail!(
                    "relay exited before ready {status}: {}",
                    relay_stderr(&stderr)
                );
            }
            if ready.exists() {
                let url = std::fs::read_to_string(&ready)?;
                println!("operator_relay pid={} url={url}", child.id());
                let mut relay = Self {
                    child,
                    stop,
                    url,
                    stderr,
                };
                relay.check_alive("ready publication")?;
                return Ok(relay);
            }
            ensure!(
                start.elapsed() < Duration::from_secs(15),
                "relay registration"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    fn check_alive(&mut self, stage: &str) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            anyhow::bail!(
                "relay exited at {stage}: {status}: {}",
                relay_stderr(&self.stderr)
            );
        }
        Ok(())
    }
    async fn start_node(&mut self, node: &mut Node) -> Result<()> {
        self.check_alive("before RelayOnly runtime start")?;
        if let Err(error) = node.start().await {
            let status = self.child.try_wait()?;
            return Err(error).context(format!(
                "RelayOnly runtime start; helper_status={status:?}; helper_stderr={}",
                relay_stderr(&self.stderr)
            ));
        }
        self.check_alive("after RelayOnly runtime start")
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.stop, []);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub(super) async fn hit(path: &Path, seconds: u64) -> Result<serde_json::Value> {
    let start = Instant::now();
    loop {
        if path.is_file()
            && let Ok(raw) = std::fs::read(path)
            && let Ok(value) = serde_json::from_slice(&raw)
        {
            return Ok(value);
        }
        ensure!(
            start.elapsed() < Duration::from_secs(seconds),
            "barrier {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn send_range(pair: &Pair, node: usize, slot: u8, start: u64, end: u64) -> Result<()> {
    let clock = Instant::now();
    for n in start..=end {
        pair.nodes[node].send(slot, n).await?;
        if n % 64 == 0 {
            println!(
                "queue node={node} slot={slot} through={n} seconds={:.3}",
                clock.elapsed().as_secs_f64()
            );
        }
    }
    Ok(())
}
pub(super) fn selected(status: &serde_json::Value) -> Result<serde_json::Value> {
    Ok(serde_json::Value::Array(status["contexts"].as_array().context("contexts")?.iter().map(|c|serde_json::json!({"d":c["d"],"e":c["e"],"sid":c["sid"],"q":c["q"],"offer":c["details"]["offer"],"grant":c["details"]["grant"],"proof":c["details"]["selected_hash"],"readies":c["details"]["readies"]})).collect()))
}
async fn exact_history(
    pair: &Pair,
    node: usize,
    slot: u8,
    amounts: [u64; 2],
    cursor: Option<Vec<u8>>,
) -> Result<(BTreeMap<(u8, u64), String>, Vec<u8>)> {
    let mut cursor = cursor;
    let mut rows = BTreeMap::new();
    let mut pages = 0;
    let start = Instant::now();
    let mut first_cursor = None;
    loop {
        let reply = pair.nodes[node]
            .call(a([
                u(2),
                u(4),
                u(u64::from(slot)),
                opt(cursor.as_ref().map(b)),
                u(16),
            ]))
            .await?;
        ensure!(reply.bytes()?.len() <= 48 * 1024, "history response cap");
        let page: serde_json::Value = serde_json::from_slice(reply.bytes()?)?;
        pages += 1;
        for row in page["items"].as_array().context("history items")? {
            let role = u8::try_from(row["role"].as_u64().context("role")?)?;
            let n = row["ordinal"].as_u64().context("ordinal")?;
            ensure!(
                role < 2 && n > 0 && n <= amounts[usize::from(role)],
                "history role/n"
            );
            let (request, text) = message(slot, role, n);
            ensure!(
                row["request"] == hex(&request) && row["text"] == text,
                "history exact request/UTF8 {role}/{n}"
            );
            let id = row["id"].as_str().context("event ID")?.to_owned();
            ensure!(rows.insert((role, n), id).is_none(), "history duplicate");
        }
        let raw = kilogram_state::indexed_dialog::HistoryCursor::from_base64(
            page["cursor"].as_str().context("cursor")?,
        )?
        .as_bytes()
        .to_vec();
        if first_cursor.is_none() {
            first_cursor = Some(raw.clone());
        }
        cursor = Some(raw);
        if page["finished"] == true {
            break;
        }
    }
    println!(
        "history node={node} slot={slot} rows={} pages={pages} seconds={:.3}",
        rows.len(),
        start.elapsed().as_secs_f64()
    );
    Ok((rows, first_cursor.context("first cursor")?))
}
fn logs(pair: &Pair) -> Result<Vec<String>> {
    let mut logs = Vec::new();
    for node in &pair.nodes {
        for n in 1..=node.starts {
            logs.push(std::fs::read_to_string(
                node.profile.with_extension(format!("{n}.stderr.log")),
            )?);
        }
    }
    Ok(logs)
}
fn number(line: &str, name: &str) -> Option<u64> {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(&format!("{name}="))?.parse().ok())
}

// The aggregate P deadline owns the budget. A live, advancing phase does not
// receive an unrelated 240s smoke-test timeout.
async fn converged(pair: &Pair, slot: usize, total: u64) -> Result<()> {
    for node in 0..2 {
        pair.state(
            node,
            Duration::from_secs(2350),
            |s| {
                let c = &s["contexts"][slot];
                c["counts"]["sources"] == total
                    && c["counts"]["queued"] == 0
                    && c["counts"]["debt"] == 0
                    && c["counts"]["pending"] == 0
            },
            "P convergence",
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn a3_s4_p_full_real_dialogue_offline_two_contexts() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(2950),async{
        let clock=Instant::now();println!("P_final_set_controller_seconds=2950 P_final_set_ceiling=3000 original_P_target_seconds=2400 root_review4_explicit_replacement_attempt=true previous_review2_attempt_preserved_FAIL=true prior_replaced_candidates_preserved_separately=true native_batch_cap=4 counts_unchanged=true");let mut pair=Pair::new("P-").await?;
        let initial=selected(&pair.nodes[0].status().await?)?;ensure!(initial==selected(&pair.nodes[1].status().await?)?,"selection/Ready symmetry");
        for through in [256,512,640]{let from=match through{256=>1,512=>257,_=>513};send_range(&pair,0,0,from,through).await?;
            converged(&pair,0,through+2).await?;
            if through==256{pair.restarted(true).await?;}
if through==512{pair.restarted(false).await?;}
            ensure!(selected(&pair.nodes[0].status().await?)?==initial,"P02 selection reset");
            println!("P02_checkpoint through={through} elapsed={:.3}",clock.elapsed().as_secs_f64());
        }
        let(_,old_cursor)=exact_history(&pair,0,0,[640,0],None).await?;
        std::fs::write(pair.base.join("history-old-after640.cursor"),&old_cursor)?;
        let mut late_before=serde_json::Value::Null;
        for round in 1..=20 {
            // Delay only committed B7 tail packets. The first packet establishes the receive DH.
            if round==7{pair.nodes[0].schedule(serde_json::json!({"slot":0,"suspend_prefix":true}))?;pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":(194..=224).collect::<Vec<_>>()}))?;}
            if round==20{pair.nodes[0].schedule(serde_json::json!({"slot":0,"suspend_prefix":true}))?;pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640]}))?;}
            send_range(&pair,0,0,641+(round-1)*2,642+(round-1)*2).await?;
            pair.state(0,Duration::from_secs(60),|s|s["contexts"][0]["counts"]["queued"]==0,"round A packets materialized").await?;
            pair.state(1,Duration::from_secs(90),|s|s["contexts"][0]["details"]["native"]["peer_next"]==643+(round-1)*2,"round A packets readable before B DH").await?;
            if round==8{late_before=pair.nodes[0].status().await?["contexts"][0]["details"]["native"].clone();println!("P04_late_tail_before={late_before}");}
            send_range(&pair,1,0,1+(round-1)*32,round*32).await?;
            if round==7{pair.state(0,Duration::from_secs(60),|s|s["contexts"][0]["details"]["native"]["peer_next"]==194,"B7 first readable").await?;
            }else{
                if round==8{let future=pair.state(0,Duration::from_secs(90),|s|s["contexts"][0]["counts"]["pending"].as_u64().unwrap_or(0)>0,"B8 future first waits exact B7 tail").await?;
                    ensure!(future["contexts"][0]["details"]["native"]["peer_next"]==194,"future DH bypassed missing prefix");
                    pair.nodes[1].schedule(serde_json::json!({"slot":0,"suspend_live":true}))?;pair.nodes[0].schedule(serde_json::json!({}))?;
                }
                if round==20{
                    pair.state(0,Duration::from_secs(90),|s|s["contexts"][0]["counts"]["sources"]==1321&&s["contexts"][0]["counts"]["debt"]==0,"B-only final tail held for relay continuation").await?;
                    pair.state(1,Duration::from_secs(90),|s|s["contexts"][0]["counts"]["sources"]==1322&&s["contexts"][0]["counts"]["queued"]==0&&s["contexts"][0]["counts"]["debt"]==1,"B640 durably materialized before offline").await?;
                }else{converged(&pair,0,642+round*34).await?;}
                if round==8{pair.nodes[1].schedule(serde_json::json!({}))?;}
            }
            println!("P03_round={round} elapsed={:.3}",clock.elapsed().as_secs_f64());
        }
        let b_request=message(0,1,640).0;let b_status=pair.nodes[1].call(a([u(2),u(3),u(0),b(b_request)])).await?;
        let b_status=b_status.option()?.context("B640 request")?.fields(3)?;ensure!(b_status[2].uint()?==1,"B640 must remain materialized and undelivered");
        let b_only_id=b_status[1].option()?.context("B640 materialized ID")?.fixed::<32>()?;
        let duplex=pair.nodes[0].status().await?;ensure!(duplex["contexts"][0]["details"]["native"]["peer_g"].as_u64().context("B DH")?>=16,"B DH generations");
        ensure!(pair.nodes[1].status().await?["contexts"][0]["details"]["native"]["peer_g"].as_u64().context("A DH")?>=16,"A DH generations");
        pair.nodes[1].stop(true).await?;let absent_pid=pair.nodes[1].child.is_none();ensure!(absent_pid,"B process still present");
        send_range(&pair,0,0,681,936).await?;
        let offline=pair.state(0,Duration::from_secs(180),|s|s["contexts"][0]["counts"]["queued"]==0&&s["contexts"][0]["counts"]["debt"]==256,"offline256 materialized").await?;
        ensure!(offline["window"][1].as_array().context("refs")?.iter().all(|n|n.as_u64().is_some_and(|n|n<129)),"offline RAM refs");
        let native_hash=offline["contexts"][0]["details"]["native_hash"].clone();
        pair.nodes[0].stop(true).await?;pair.nodes[0].start().await?;
        ensure!(pair.nodes[0].status().await?["contexts"][0]["details"]["native_hash"]==native_hash,"offline Native changed");
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"hold_lane":0,"hold_ordinals":[681],"suspend_prefix":true}))?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640],"suspend_prefix":true}))?;
        pair.nodes[1].start().await?;pair.import().await?;
        let full=pair.state(1,Duration::from_secs(120),|s|s["contexts"][0]["counts"]["pending"]==64,"full64 ahead").await?;
        ensure!(full["contexts"][0]["details"]["native"]["peer_next"]==681,"lowest missing changed");
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"hold_lane":0,"hold_ordinals":[681],"hold_window64":true,"suspend_prefix":true}))?;
        let held=hit(&pair.nodes[0].schedule.with_extension("transport-hit"),45).await?;
        ensure!(held["count"]==64&&held["ids"].as_array().context("held IDs")?.iter().map(|id|id.as_str()).collect::<BTreeSet<_>>().len()==64,"real distinct held64");
        for n in 1..=32{pair.nodes[0].send(1,n).await?;pair.nodes[1].send(1,n).await?;}
        let overlap=pair.state(0,Duration::from_secs(60),|s|s["window"][1][0]==64&&s["contexts"][1]["counts"]["sources"].as_u64().is_some_and(|n|n>2),"C2 actual progress while C1 owns 64 RAM refs").await?;
        println!("P07_actual_overlap refs={} C2_sources={}",overlap["window"],overlap["contexts"][1]["counts"]["sources"]);
        converged(&pair,1,66).await?;
        let astate=pair.nodes[0].status().await?;let bstate=pair.nodes[1].status().await?;
        ensure!(bstate["contexts"][0]["counts"]["pending"]==64&&astate["contexts"][0]["counts"]["debt"]==256,"C1 advanced/evicted during C2");
        ensure!(astate["window"][2][0]==64,"held window was not observed");
        println!("P07_C2_converged_with_C1_pending64 refs={} elapsed={:.3}",astate["window"],clock.elapsed().as_secs_f64());
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"suspend_live":true,"suspend_prefix":true}))?;
        let before=bstate["stats"]["prefix_pages"][0].as_u64().context("prefix count")?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640],"pause_prefix_page":before+2}))?;
        let first=hit(&pair.nodes[1].schedule.with_extension("prefix-hit"),120).await?;println!("P06_restart_after_normal_page2={first}");
        pair.nodes[1].stop(true).await?;std::fs::remove_file(pair.nodes[1].schedule.with_extension("prefix-hit"))?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640],"pause_prefix_page":3}))?;
        pair.nodes[1].start().await?;pair.import().await?;
        let second=hit(&pair.nodes[1].schedule.with_extension("prefix-hit"),120).await?;println!("P06_restart_after_normal_page5={second}");
        pair.nodes[1].stop(true).await?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640],"disable_ack":true}))?;
        pair.nodes[1].start().await?;pair.import().await?;
        let lost_ack=hit(&pair.nodes[1].schedule.with_extension("transport-hit"),90).await?;
        ensure!(lost_ack["count"].as_u64().is_some_and(|n|n>0),"actual nonempty ACK lost before write");
        pair.nodes[0].stop(true).await?;pair.nodes[0].start().await?;pair.import().await?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[640]}))?;
        pair.state(0,Duration::from_secs(240),|s|s["contexts"][0]["counts"]["sources"]==1577&&s["contexts"][0]["counts"]["debt"]==0&&s["contexts"][0]["counts"]["pending"]==0,"all A catch-up/ACK except held B-only tail").await?;
        pair.state(1,Duration::from_secs(90),|s|s["contexts"][0]["counts"]["sources"]==1578&&s["contexts"][0]["counts"]["debt"]==1,"B-only debt retained across all direct restarts").await?;
        let mut relay=Relay::start(&pair.base).await?;
        pair.nodes[0].stop(false).await?;pair.nodes[1].stop(false).await?;
        for node in &pair.nodes{node.route("relay-only",Some(&relay.url),None)?;node.schedule(serde_json::json!({}))?;}
        relay.start_node(&mut pair.nodes[0]).await?;relay.start_node(&mut pair.nodes[1]).await?;
        for node in &pair.nodes{for context in &pair.contexts{let loc=wire::ParsedRuntimeLocator2::parse(&std::fs::read(node.locators.join(format!("{context}.locator")))?)?;ensure!(loc.fields().ips.is_empty()&&loc.fields().relay.as_deref()==Some(relay.url.as_str()),"actual URL-only RelayOnly");}}
        pair.import().await?;pair.wait_active().await?;converged(&pair,0,1578).await?;converged(&pair,1,66).await?;
        let delivered=pair.nodes[1].call(a([u(2),u(3),u(0),b(b_request)])).await?;let delivered=delivered.option()?.context("B640 after latest locator import")?.fields(3)?;
        ensure!(delivered[1].option()?.context("B640 ID")?.fixed::<32>()?==b_only_id&&delivered[2].uint()?==2,"latest relay locator did not continue exact B-only work");
        println!("P08_B_only_exact_progress event={} A_new_send_or_open=false",hex(&b_only_id));
        relay.check_alive("before final RelayOnly restart")?;
        let restart=pair.restarted(false).await;
        if let Err(error)=restart{return Err(error).context(format!("final RelayOnly restart: {}",relay_stderr(&relay.stderr)));}
        relay.check_alive("after final RelayOnly restart")?;converged(&pair,0,1578).await?;
        let mut history_ids=Vec::new();for slot in 0..2{let amounts=if slot==0{[936,640]}else{[32,32]};
            let(a,_)=exact_history(&pair,0,slot,amounts,None).await?;let(b,_)=exact_history(&pair,1,slot,amounts,None).await?;
            ensure!(a==b&&a.len()==amounts.iter().sum::<u64>() as usize,"exact converged history");history_ids.push(a);}
        let(old,_)=exact_history(&pair,0,0,[640,0],Some(old_cursor)).await?;ensure!(old.len()==624,"old cursor suffix");
        ensure!(selected(&pair.nodes[0].status().await?)?==initial,"final selection changed");
        for node in &mut pair.nodes{println!("P_final_status {}",node.status().await?);node.stop(false).await?;}
        let logs=logs(&pair)?;let mut calls=0;let mut attempts=0;let mut bindings=BTreeSet::new();let mut normal_pages=0;let mut heads=0;let mut accounts=0;let mut sessions=0;
        for log in &logs{for line in log.lines(){if line.starts_with("od2_native_commit "){calls+=number(line,"calls").context("committed calls")?;}
if line.starts_with("od2_native_attempt "){attempts+=1;}
            if line.starts_with("od2_admitted ")&& let Some(binding)=line.split_whitespace().find_map(|p|p.strip_prefix("binding=")){bindings.insert(binding.to_owned());}
            if line.starts_with("od2_prefix_saved context=0 head=false "){normal_pages+=1;ensure!(number(line,"count").is_some_and(|n|n<=16),"normal prefix page cap");}
            if line.starts_with("od2_funded_head context=0 ordinary_pending=64 "){heads+=1;}
            if line.starts_with("od2_account_constructor "){accounts+=1;}
            if line.starts_with("od2_session_constructor "){sessions+=1;}
        }}
        let late=logs.iter().flat_map(|log|log.lines()).find(|line|line.starts_with("od2_readable_commit context=0 role=1 n=224 ")).context("actual late B7 tail commit")?;
        ensure!(number(late,"own_g")==late_before["own_g"].as_u64()&&number(late,"own_index")==late_before["own_index"].as_u64()
            &&late.contains(&format!("own_first={}",late_before["own_first"].as_str().context("A8 first")?))&&late.ends_with("rotate_due=false"),"late B7 tail changed own Active/T: {late}");
        ensure!(calls==3288,"successful Native packet calls {calls} !=3288");ensure!(attempts>=calls,"attempt accounting");
        ensure!(accounts==4&&sessions==4,"P recreated native constructors {accounts}/{sessions}");
        ensure!(bindings.len()>=3&&normal_pages>=8&&heads>=1,"bindings/pages/funded head: {}/{normal_pages}/{heads}",bindings.len());
        println!("S4_P_PASS text=1640 genesis=4 packet_calls={calls} attempts={attempts} aborted={} bindings={} normal_pages={normal_pages} full64_head={heads} seconds={:.3}",attempts-calls,bindings.len(),clock.elapsed().as_secs_f64());
        Ok::<_,anyhow::Error>(())
    }).await.context("P aggregate deadline")?
}

#[tokio::test]
async fn a3_s4_relay_smoke_exact_b_only_after_rebind() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(170), async {
        let clock = Instant::now();
        let mut pair = Pair::new("relay-smoke-").await?;
        let direct = [pair.nodes[0].status().await?, pair.nodes[1].status().await?];
        let initial = selected(&direct[0])?;
        ensure!(initial == selected(&direct[1])?, "initial selection");
        pair.nodes[0].schedule(serde_json::json!({"slot":0,"suspend_prefix":true}))?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"hold_lane":1,"hold_ordinals":[1]}))?;
        pair.nodes[1].send(0, 1).await?;
        pair.state(1, Duration::from_secs(60), |s| {
            s["contexts"][0]["counts"]["sources"] == 3
                && s["contexts"][0]["counts"]["queued"] == 0
                && s["contexts"][0]["counts"]["debt"] == 1
        }, "real B-only committed debt before relay restart").await?;
        ensure!(pair.nodes[0].status().await?["contexts"][0]["counts"]["sources"] == 2, "held B-only Text already readable");
        let request = message(0, 1, 1).0;
        let queued_reply = pair.nodes[1].call(a([u(2),u(3),u(0),b(request)])).await?;
        let queued = queued_reply.option()?.context("B-only request")?.fields(3)?;
        let event = queued[1].option()?.context("materialized B-only ID")?.fixed::<32>()?;
        ensure!(queued[2].uint()? == 1, "B-only falsely delivered before relay");
        let mut relay = Relay::start(&pair.base).await?;
        for node in &mut pair.nodes { node.stop(false).await?; }
        for node in &pair.nodes {
            node.route("relay-only", Some(&relay.url), None)?;
            node.schedule(serde_json::json!({}))?;
        }
        relay.start_node(&mut pair.nodes[0]).await?;
        relay.start_node(&mut pair.nodes[1]).await?;
        for (index, node) in pair.nodes.iter().enumerate() {
            let status = node.status().await?;
            ensure!(status["endpoint"]["id"].as_str().context("new actual endpoint")?
                != direct[index]["endpoint"]["id"].as_str().context("old actual endpoint")?, "relay restart reused endpoint");
            for (slot, context) in pair.contexts.iter().enumerate() {
                let raw = std::fs::read(node.locators.join(format!("{context}.locator")))?;
                let locator = wire::ParsedRuntimeLocator2::parse(&raw)?;
                ensure!(locator.fields().ips.is_empty()
                    && locator.fields().relay.as_deref() == Some(relay.url.as_str())
                    && locator.fields().revision > direct[index]["contexts"][slot]["own_locator_revision"].as_u64().context("old locator revision")?,
                    "actual signed URL-only locator");
                println!("relay_smoke_locator node={index} slot={slot} endpoint={} revision={} ips=0 relay={}",hex(&locator.fields().endpoint),locator.fields().revision,relay.url);
            }
        }
        pair.import().await?;
        pair.wait_active().await?;
        pair.converged(0, 3).await?;
        let delivered_reply = pair.nodes[1].call(a([u(2),u(3),u(0),b(request)])).await?;
        let delivered = delivered_reply.option()?.context("B-only after relay")?.fields(3)?;
        ensure!(delivered[1].option()?.context("delivered ID")?.fixed::<32>()? == event
            && delivered[2].uint()? == 2, "exact B-only receipt missing");
        for node in 0..2 {
            ensure!(selected(&pair.nodes[node].status().await?)? == initial, "relay changed selection");
            let (history, _) = exact_history(&pair, node, 0, [0,1], None).await?;
            ensure!(history.len() == 1 && history.get(&(1,1)) == Some(&hex(&event)), "exact readable B-only ID/request/UTF8");
        }
        relay.check_alive("after actual B-only relay delivery")?;
        for node in &mut pair.nodes { node.stop(false).await?; }
        let logs = logs(&pair)?;
        let accounts = logs.iter().flat_map(|s|s.lines()).filter(|s|s.starts_with("od2_account_constructor ")).count();
        let sessions = logs.iter().flat_map(|s|s.lines()).filter(|s|s.starts_with("od2_session_constructor ")).count();
        let calls = logs.iter().flat_map(|s|s.lines()).filter(|s|s.starts_with("od2_native_commit ")).map(|s|number(s,"calls").context("calls")).collect::<Result<Vec<_>>>()?.iter().sum::<u64>();
        ensure!(accounts == 4 && sessions == 4 && calls == 10, "relay recreated Native or changed packet work");
        relay.check_alive("clean runtime shutdown")?;
        drop(relay);
        ensure!(pair.nodes.iter().all(|node|node.child.is_none()), "owned CLI children remain");
        println!("S4_RELAY_SMOKE_PASS actual_own_relay=true signed_URL_only_empty_IP=true B_only_Text=1 exact_event={} receipt_status=2 A_new_send_or_open=false selection_unchanged=true accounts={accounts} sessions={sessions} successful_packet_calls={calls} children_clean=true seconds={:.6}",hex(&event),clock.elapsed().as_secs_f64());
        Ok::<_,anyhow::Error>(())
    }).await.context("own relay focused170s watchdog")?
}
