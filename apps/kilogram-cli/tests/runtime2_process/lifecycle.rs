use super::*;
use kilogram_identity::DeviceIdentity;
use kilogram_transport_iroh::{RoutePolicy, ordered2 as transport};
use positive::hit;
fn fixed_port(used: &mut Vec<u16>) -> Result<String> {
    loop {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        if !used.contains(&port) {
            used.push(port);
            return Ok(format!("127.0.0.1:{port}"));
        }
    }
}
fn frozen(node: &Node) -> Result<serde_json::Value> {
    let mut r = faults::cold(node)?;
    let mut s = r.snapshot(0)?;
    s.head.generation = 0; // Locator commits advance the operation generation, never Native/identity state.
    Ok(
        serde_json::json!({"head":s.head,"selection":wire::hash(b"",&postcard::to_allocvec(&s.selection)?),"sequence":s.profile.next_author_sequence,"counts":r.counts(0)?,"source":r.source(0,1,1)?.map(|r|wire::hash(b"",&r.raw))}),
    )
}
fn locator(node: &Node, c: &str) -> Result<wire::ParsedRuntimeLocator2> {
    Ok(wire::ParsedRuntimeLocator2::parse(&std::fs::read(
        node.locators.join(format!("{c}.locator")),
    )?)?)
}
async fn reject_locator(node: &Node, raw: Vec<u8>, reason: &str) -> Result<()> {
    let error = node
        .call(a([u(2), u(5), u(0), b(raw)]))
        .await
        .err()
        .context("locator must be refused")?;
    ensure!(
        format!("{error:#}").contains(reason),
        "wrong locator refusal {error:#}"
    );
    Ok(())
}
#[tokio::test]
async fn a3_s4_f11_repeated_locator_kills_export_retry_b_only() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(180),async{
        let mut pair=Pair::new("F-locator-").await?;pair.nodes[0].schedule(serde_json::json!({"slot":0,"suspend_prefix":true}))?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"suspend_live":true}))?;pair.nodes[1].send(0,1).await?;
        pair.state(1,Duration::from_secs(20),|s|s["contexts"][0]["counts"]["queued"]==0&&s["contexts"][0]["counts"]["debt"]==1,"pre-existing materialized B-only debt").await?;
        let original=locator(&pair.nodes[1],&pair.contexts[0])?;pair.nodes[1].stop(true).await?;
        let before=frozen(&pair.nodes[1])?;let signer={let mut r=faults::cold(&pair.nodes[1])?;DeviceIdentity::from_secret_bytes(*r.owner_material()?.signing)};
        let mut used=vec![];let mut artifacts=vec![];
        for _ in 0..2{
            let port=fixed_port(&mut used)?;pair.nodes[1].route("direct-only",None,Some(&port))?;
            let marker=pair.nodes[1].schedule.with_extension("hit");if marker.exists(){std::fs::remove_file(&marker)?;}
            pair.nodes[1].schedule(serde_json::json!({"slot":0,"pause":"locator_committed","suspend_live":true}))?;pair.nodes[1].launch()?;
            let hit=hit(&marker,25).await?;ensure!(hit["pid"]==pair.nodes[1].child.as_ref().context("child")?.id()&&hit["point"]=="locator_committed","actual locator child barrier");
            ensure!(!pair.nodes[1].locators.join(format!("{}.locator",pair.contexts[0])).exists(),"old endpoint still advertised at new commit");
            pair.nodes[1].stop(true).await?;let mut r=faults::cold(&pair.nodes[1])?;let raw=r.snapshot(0)?.spec.own_locator.context("committed newer locator")?;drop(r);
            artifacts.push(wire::ParsedRuntimeLocator2::parse(&raw)?);ensure!(frozen(&pair.nodes[1])?==before,"locator kill changed Native/source/selection");println!("F11_actual_locator_kill marker={hit}");
        }
        let port=fixed_port(&mut used)?;pair.nodes[1].route("direct-only",None,Some(&port))?;pair.nodes[1].schedule(serde_json::json!({"slot":0,"suspend_live":true}))?;pair.nodes[1].start().await?;
        let latest=locator(&pair.nodes[1],&pair.contexts[0])?;artifacts.push(latest.clone());
        for (i,current) in artifacts.iter().enumerate(){ensure!(current.fields().revision==original.fields().revision+i as u64+1,"durable revisions must be r,r+1,r+2");ensure!(current.fields().endpoint!=original.fields().endpoint&&current.fields().ips.iter().any(|ip|used[i]==ip.port()),"actual endpoint/controlled port");}
        ensure!(artifacts.windows(2).all(|w|w[0].fields().endpoint!=w[1].fields().endpoint),"per-restart endpoint uniqueness");
        pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;pair.nodes[0].import(0,&pair.nodes[1],&pair.contexts).await?;
        reject_locator(&pair.nodes[0],original.raw().to_vec(),"rollback/conflict").await?;
        let mut changed=latest.fields().clone();changed.expires+=1;reject_locator(&pair.nodes[0],wire::ParsedRuntimeLocator2::sign(changed,&signer)?.raw().to_vec(),"rollback/conflict").await?;
        let mut expired=latest.fields().clone();expired.revision+=1;expired.not_before=1;expired.expires=2;reject_locator(&pair.nodes[0],wire::ParsedRuntimeLocator2::sign(expired,&signer)?.raw().to_vec(),"locator time").await?;
        pair.nodes[0].schedule(serde_json::json!({}))?;pair.converged(0,3).await?;pair.history(0,0,1).await?;
        let old_native=pair.nodes[1].status().await?["contexts"][0]["details"]["native_hash"].clone();pair.nodes[1].stop(false).await?;
        let marker=pair.nodes[1].schedule.with_extension("hit");std::fs::remove_file(&marker)?;
        pair.nodes[1].schedule(serde_json::json!({"slot":0,"pause":"locator_committed"}))?;let retry_log=pair.nodes[1].launch()?;let retry=hit(&marker,25).await?;
        let public_path=pair.nodes[1].locators.join(format!("{}.locator",pair.contexts[0]));std::fs::create_dir(&public_path)?;pair.nodes[1].schedule(serde_json::json!({}))?;pair.nodes[1].wait_ready(&retry_log).await?;
        let failed=pair.state(1,Duration::from_secs(25),|s|s["contexts"][0]["publication_ready"]==true&&s["contexts"][0]["file_exported"]==false,"same live endpoint export error").await?;
        ensure!(failed["endpoint"]["id"]==retry["public"]["endpoint"]&&failed["contexts"][0]["details"]["native_hash"]==old_native,"export error rotated native/endpoint");
        std::fs::remove_dir(&public_path)?;
        let ready=pair.state(1,Duration::from_secs(10),|s|s["contexts"][0]["file_exported"]==true,"same-process exact export retry").await?;
        let retried=locator(&pair.nodes[1],&pair.contexts[0])?;ensure!(ready["endpoint"]==failed["endpoint"]&&ready["contexts"][0]["own_locator_revision"]==failed["contexts"][0]["own_locator_revision"]&&hex(&retried.fields().endpoint)==retry["public"]["endpoint"],"export retry changed locator instance/revision");
        let exported=pair.nodes[1].call(a([u(2),u(6),u(0)])).await?;ensure!(exported.bytes()?==retried.raw(),"exact live export retry bytes");
        pair.import().await?;pair.converged(0,3).await?;for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_F11_PASS actual_kills=2 revisions_consecutive=true distinct_controlled_ports=3 B_only_materialized_before_kills=true latest_import_without_A_send=true same_live_export_retry_exact=true lower_equal_changed_expired_refused=true");Ok::<_,anyhow::Error>(())
    }).await.context("F11 locator lifecycle deadline")?
}
#[tokio::test]
async fn a3_s4_f12_actual_pre_admission_two_permits_stream_bounds() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(110),async{
        let mut pair=Pair::new("F-admission-").await?;pair.nodes[0].stop(false).await?;
        pair.state(1,Duration::from_secs(10),|s|s["resources"]["connections"]==0,"previous actual connections released").await?;
        let signer={let mut r=faults::cold(&pair.nodes[0])?;DeviceIdentity::from_secret_bytes(*r.owner_material()?.signing)};
        let route=transport::RouteConfig2{policy:RoutePolicy::DirectOnly,bind:Some("127.0.0.1:0".parse()?),relay:None};
        let endpoints=[transport::bind(&route).await?,transport::bind(&route).await?,transport::bind(&route).await?];
        for (c,endpoint) in endpoints.iter().enumerate().take(2){let old=locator(&pair.nodes[0],&pair.contexts[c])?;let mut f=old.fields().clone();f.endpoint = *endpoint.id().as_bytes();f.ips=endpoint.addr().ip_addrs().copied().collect();f.revision+=1;
            let raw=wire::ParsedRuntimeLocator2::sign(f,&signer)?.raw().to_vec();pair.nodes[1].call(a([u(2),u(5),u(c as u64),b(raw)])).await?;}
        let remote=route.remote(&locator(&pair.nodes[1],&pair.contexts[0])?)?;
        let first=endpoints[0].connect(remote.clone(),wire::ALPN).await?;let second=endpoints[1].connect(remote.clone(),wire::ALPN).await?;
        let state=pair.state(1,Duration::from_secs(10),|s|s["resources"]["connections"]==2,"two real TLS/pre-Hello permits").await?;
        ensure!(state["resources"]["peak_connections"]==2&&state["resources"]["peak_mailbox"].as_u64().is_some_and(|n|n<=16),"pre-admission resources");
        let third=tokio::time::timeout(Duration::from_secs(5),endpoints[2].connect(remote.clone(),wire::ALPN)).await;
        ensure!(matches!(third,Ok(Err(_))),"third TLS connection was not refused before admission");
        pair.state(1,Duration::from_secs(5),|s|s["resources"]["refused_before_spawn"].as_u64().is_some_and(|n|n>0)&&s["resources"]["connections"]==2,"pre-spawn third refusal").await?;
        let mut held=Vec::new();for _ in 0..4{let(mut send,receive)=first.open_bi().await?;send.write_all(&[0]).await?;held.push((send,receive));}
        ensure!(tokio::time::timeout(Duration::from_millis(300),first.open_bi()).await.is_err(),"more than four bidi streams admitted");
        ensure!(tokio::time::timeout(Duration::from_millis(300),first.open_uni()).await.is_err(),"unidirectional streams admitted");
        first.close(0u32.into(),b"real early close");drop(held);
        pair.state(1,Duration::from_secs(10),|s|s["resources"]["connections"]==1,"early close releases permit").await?;
        pair.state(1,Duration::from_secs(35),|s|s["resources"]["connections"]==0,"absolute admission deadline releases second permit").await?;
        ensure!(second.close_reason().is_some(),"deadline did not close actual connection");
        let again=endpoints[0].connect(remote,wire::ALPN).await?;pair.state(1,Duration::from_secs(5),|s|s["resources"]["connections"]==1,"new valid TLS after release").await?;again.close(0u32.into(),b"done");
        pair.state(1,Duration::from_secs(10),|s|s["resources"]["connections"]==0,"last connection released").await?;
        for endpoint in endpoints{endpoint.close().await;}pair.nodes[0].start().await?;
let current=std::fs::read(pair.nodes[0].locators.join(format!("{}.locator",pair.contexts[0])))?;let conflict=pair.nodes[1].call(a([u(2),u(5),u(0),b(current)])).await.err().context("parent endpoint locator high-water must reject same revision changed endpoint")?;ensure!(format!("{conflict:#}").contains("rollback/conflict"),"actual endpoint revision gate");
pair.nodes[0].stop(false).await?;pair.nodes[0].start().await?;pair.import().await?;pair.wait_active().await?;
        pair.nodes[0].send(0,1).await?;pair.converged(0,3).await?;for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_F12_PASS actual_pre_Hello_TLS=2 third_refused_before_spawn=true bidi_limit=4 uni_limit=0 close_and_absolute_deadline_release=true fresh_full_admission_after_release=true");Ok::<_,anyhow::Error>(())
    }).await.context("F12 admission deadline")?
}
