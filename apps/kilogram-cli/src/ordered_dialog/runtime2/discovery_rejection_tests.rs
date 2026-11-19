//! Fresh Actors2 profiles; never reads the preserved field profiles.
use super::*;
use discovery::NetworkGeneration2;
use kilogram_transport_iroh::RoutePolicy;
use std::net::{Ipv4Addr, SocketAddr};

#[tokio::test]
async fn t13reject_owner_preserves_errors_revisions_pair_and_leases() -> Result<()> {
    let fixtures = Actors2::new(false).await?;
    let id = fixtures.endpoints[1].id();
    fixtures.handles[1].call(move |owner| {
        let local = Ipv4Addr::new(192,168,44,2);
        let peer = Ipv4Addr::new(192,168,44,1);
        owner.route = transport::FreshRoute2::new(transport::RouteConfig2 {
            policy: RoutePolicy::DirectOnly, bind: Some((local,0).into()), relay: None,
        },Some(peer))?;
        let address = iroh::EndpointAddr::new(id).with_ip_addr(SocketAddr::from((local,41000)));
        owner.test_lan_advertisement = Some(address.clone());
        for c in 0..2 { owner.rotate_locator(c,[27;32],&address)?; }
        let baseline = [owner.snapshot(0)?.spec, owner.snapshot(1)?.spec];
        let generation = owner.repository.generation();
        let mut errors = Vec::new();
        for enabled in [false,true] {
            owner.diagnostics = Arc::new(diagnostics::Counters::new(enabled));
            owner.discovery_quota = discovery::DiscoveryQuota2::default();
            let epoch = if enabled {3} else {1};
            owner.enable_lan_discovery(NetworkGeneration2(epoch))?;
            let old = owner.capture_lan_discovery(NetworkGeneration2(epoch))?;
            owner.enable_lan_discovery(NetworkGeneration2(epoch+1))?;
            let observed = owner.capture_lan_discovery(NetworkGeneration2(epoch+1))?;
            let stale = owner.accept_lan_discovery(&old,&[]).expect_err("old generation");
            let scope = owner.commit_discovery_pair(&observed,&[],|| anyhow::bail!("synthetic clock unavailable"),|_,_,_,_,_,_| anyhow::bail!("unexpected write")).expect_err("clock");
            let mut frame_errors = Vec::new();
            for _ in 0..8 {
                frame_errors.push(owner.accept_lan_discovery(&observed,&[]).expect_err("frame").to_string());
            }
            let quota = owner.accept_lan_discovery(&observed,&[]).expect_err("quota");
            ensure!(frame_errors.iter().all(|e| e == "OD2 discovery datagram size"), "{frame_errors:?}");
            errors.push((stale.to_string(),scope.to_string(),frame_errors,quota.to_string()));
            let snapshot = owner.diagnostics.snapshot();
            let rejected = &snapshot["discovery_rejections"];
            for (key,count) in [("GenerationFreshness",1),("LocalScope",1),("FrameSize",8),("ValidationQuota",1)] {
                ensure!(rejected[key] == if enabled { count } else { 0 });
            }
            ensure!(rejected.as_object().context("histogram")?.values().map(|v|v.as_u64().unwrap()).sum::<u64>() == if enabled {11} else {0});
            for (c,before) in baseline.iter().enumerate() {
                let after = owner.snapshot(c as u8)?;
                ensure!(after.spec.peer_locator_revision == before.peer_locator_revision);
                ensure!(after.spec.peer_locator == before.peer_locator);
                ensure!(owner.leases[c].is_none());
            }
            ensure!(owner.repository.generation() == generation);
            ensure!(owner.listener_pair((peer,42000).into()).is_err());
            ensure!(owner.diagnostics.snapshot()["counters"]["LocatorCommitted"] == 0);
        }
        ensure!(errors[0] == errors[1]);
        println!("T13REJECT_OWNER diagnostics_on_off_equivalent=true peer_state_unchanged=true pair_closed=true leases=0");
        Ok(())
    }).await
}
