use super::*;
use kilogram_ratchet::ordered_v2::Guard2;
use native::Native2;
use owner::{Owner2, Purpose2};

pub(super) fn pair_credentials(
    selection: &db::RuntimeSelection,
) -> Result<[wire::RootVerifiedCredential; 2]> {
    let pair = wire::decode(&selection.initial_pair, 192 * 1024)?;
    let p = pair.fields(6)?;
    let credential = |raw: &Value| -> Result<wire::RootVerifiedCredential> {
        let hello = wire::ParsedFrame2::parse(raw.bytes()?)?;
        Ok(
            wire::ParsedCredentialBundle::parse(hello.body().value().fields(12)?[7].bytes()?)?
                .verify_root()?,
        )
    };
    Ok([credential(&p[2])?, credential(&p[3])?])
}
pub(super) fn selection(
    s: &db::RuntimeSnapshot,
    chosen: &db::RuntimeSelection,
    sid: Option<String>,
) -> Result<wire::StaticSelection2> {
    let peers = pair_credentials(chosen)?;
    ensure!(
        peers.iter().map(|c| c.cert_ref()).collect::<Vec<_>>() == s.spec.certs,
        "selected certificate pins"
    );
    let proof = wire::ParsedModeProof2::parse(&chosen.proof)?;
    ensure!(
        proof.origin() == &wire::Origin::Fresh,
        "UnsupportedExistingRuntime"
    );
    Ok(wire::StaticSelection2::new(
        proof.verify(&wire::SelectionEvidence2 {
            peers,
            membership: s.spec.membership.clone(),
            initial_pair: Some(chosen.initial_pair.clone()),
            previous: None,
        })?,
        sid,
    )?)
}
fn proof(chosen: &mut db::RuntimeSelection, c: Id) -> Result<()> {
    chosen.proof = wire::encode(
        &a([
            u(2),
            u(0),
            b(c),
            b(&chosen.selected_consent),
            b(&chosen.offer),
            b(&chosen.grant),
        ]),
        wire::MAX_PROOF,
    )?;
    Ok(())
}
impl Owner2 {
    pub(super) fn restore_controls(&mut self, c: u8, s: &db::RuntimeSnapshot) -> Result<()> {
        let peers = s
            .spec
            .credentials
            .iter()
            .map(|raw| wire::ParsedCredentialBundle::parse(raw)?.verify_root())
            .collect::<wire::Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| anyhow::anyhow!("peer pair"))?;
        wire::verify_membership(s.head.context, &peers, &s.spec.membership)?;
        if s.head.phase == 0 {
            ensure!(
                s.selection.offer.is_empty()
                    && s.selection.grant.is_empty()
                    && s.selection.proof.is_empty()
                    && s.selection.initial_pair.is_empty(),
                "enrolled control shape"
            );
            if !s.selection.selected_consent.is_empty() {
                let old = wire::ParsedConsent2::parse(&s.selection.selected_consent)?;
                old.verify(&peers)?;
                ensure!(
                    old.c() == s.head.context && old.origin() == &wire::Origin::Fresh,
                    "historical support scope"
                );
            }
            return Ok(());
        }
        let peers = pair_credentials(&s.selection)?;
        ensure!(
            peers.iter().map(|p| p.cert_ref()).collect::<Vec<_>>() == s.spec.certs,
            "historical pair certificate pins"
        );
        let consent = wire::ParsedConsent2::parse(&s.selection.selected_consent)?;
        let (t, _) = wire::verify_initial_pair2(
            s.head.context,
            &consent,
            &s.selection.initial_pair,
            &peers,
            &s.spec.membership,
        )?;
        let offer = wire::ParsedControl2::parse(&s.selection.offer)?;
        offer.require_kind(1)?;
        offer.verify(peers[0].certificate().device_id())?;
        let body = offer.body().fields(8)?;
        ensure!(
            s.head.offer == Some(offer.id())
                && body[0].fixed::<32>()? == s.head.context
                && body[1].fixed::<32>()? == s.spec.certs[0]
                && body[2].fixed::<32>()? == s.spec.certs[1]
                && body[3].fixed::<32>()? == t,
            "restored Offer scope"
        );
        if s.head.phase == 1 {
            ensure!(
                s.selection.grant.is_empty() && s.selection.proof.is_empty(),
                "OfferedA controls"
            );
            return Ok(());
        }
        let selected = selection(s, &s.selection, s.head.sid.clone())?;
        let parsed = wire::decode(&s.selection.proof, wire::MAX_PROOF)?;
        ensure!(
            parsed.fields(6)?[3].bytes()? == s.selection.selected_consent
                && s.head.dialog == Some(selected.d())
                && s.head.epoch == Some(selected.e())
                && s.head.mode_ref == Some(selected.proof().q())
                && s.head.grant == Some(wire::ParsedControl2::parse(&s.selection.grant)?.id()),
            "restored selected proof Head"
        );
        if s.head.phase == 2 {
            return Ok(());
        }
        let originals = pair_credentials(&s.selection)?;
        let mut genesis = [None, None];
        for role in 0..2 {
            if let Some(source) = self.repository.source(c, role as u8, 0)? {
                let event = wire::ParsedEvent2::parse(&source.raw)?;
                event.clone().verify(&selected, &originals[role])?;
                genesis[role] = Some(event.id());
            }
        }
        for (role, peer) in peers.iter().enumerate() {
            if let Some(raw) = &s.head.ready[role] {
                wire::ParsedControl2::parse(raw)?.verify_ready(
                    &selected,
                    role as u8,
                    [
                        genesis[0].context("Ready A0")?,
                        genesis[1].context("Ready B0")?,
                    ],
                    peer,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn offer(&mut self, c: u8) -> Result<(Vec<u8>, Vec<u8>)> {
        let (s, _) = self.require(c, true)?;
        ensure!(s.head.role == 0, "A role required");
        if s.head.phase > 0 {
            return Ok((s.selection.offer, s.selection.initial_pair));
        }
        let lease = self.leases[usize::from(c)].as_ref().context("admission")?;
        let pair = lease.pair.clone();
        let p = wire::decode(&pair, 192 * 1024)?;
        let t = p.fields(6)?[1].fixed::<32>()?;
        let (private, _) = Native2::fresh(false)?;
        eprintln!("od2_account_constructor context={c} role=0");
        let offer = wire::ParsedControl2::sign(
            1,
            &wire::encode(
                &a([
                    b(s.head.context),
                    b(s.spec.certs[0]),
                    b(s.spec.certs[1]),
                    b(t),
                    b(shared_admin::random::<32>()?),
                    b(private.account.curve_key()),
                    u(1),
                    u(2),
                ]),
                1024,
            )?,
            &self.device,
        )?;
        let chosen = db::RuntimeSelection {
            initial_pair: pair.clone(),
            selected_consent: s.spec.consent.clone(),
            offer: offer.raw().to_vec(),
            ..Default::default()
        };
        let mut target = s.head.clone();
        target.phase = 1;
        target.offer = Some(offer.id());
        target.native_generation = target
            .native_generation
            .checked_add(1)
            .context("native generation")?;
        let raw = private.seal(&s.profile, &target, &self.encryption)?;
        let hash = wire::hash(b"", &raw);
        self.commit(
            &s,
            db::RuntimeOperation::PrepareOffer {
                context: c,
                fence: s.fence(now()?),
                selection: chosen,
                private: raw,
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        self.cache_native(c, hash, private)?;
        Ok((offer.raw().to_vec(), pair))
    }
    pub(super) fn grant(&mut self, c: u8, offer_raw: Vec<u8>, pair: Vec<u8>) -> Result<Vec<u8>> {
        let (s, current) = self.require(c, true)?;
        ensure!(s.head.role == 1, "B role required");
        let offer = wire::ParsedControl2::parse(&offer_raw)?;
        offer.require_kind(1)?;
        offer.verify(current.peers[0].certificate().device_id())?;
        if s.head.phase >= 2 {
            ensure!(
                s.selection.offer == offer_raw && s.selection.initial_pair == pair,
                "CreationConflict"
            );
            return Ok(s.selection.grant);
        }
        ensure!(s.head.phase == 0, "CreationConflict");
        let p = wire::decode(&pair, 192 * 1024)?;
        let pf = p.fields(6)?;
        let hello = wire::ParsedFrame2::parse(pf[2].bytes()?)?;
        let consent_ref = hello.body().value().fields(12)?[9].fixed::<32>()?;
        let selected_consent = if wire::hash(wire::CONSENT, &s.spec.consent) == consent_ref {
            s.spec.consent.clone()
        } else {
            ensure!(
                wire::hash(wire::CONSENT, &s.selection.selected_consent) == consent_ref,
                "NeedHistoricalConsent"
            );
            s.selection.selected_consent.clone()
        };
        let consent = wire::ParsedConsent2::parse(&selected_consent)?;
        let historical = pair_credentials(&db::RuntimeSelection {
            initial_pair: pair.clone(),
            ..Default::default()
        })?;
        ensure!(
            historical.iter().map(|p| p.cert_ref()).collect::<Vec<_>>() == s.spec.certs,
            "historical certificate pins"
        );
        let (t, _) = wire::verify_initial_pair2(
            s.head.context,
            &consent,
            &pair,
            &historical,
            &s.spec.membership,
        )?;
        let o = offer.body().fields(8)?;
        ensure!(
            o[0].fixed::<32>()? == s.head.context
                && o[1].fixed::<32>()? == s.spec.certs[0]
                && o[2].fixed::<32>()? == s.spec.certs[1]
                && o[3].fixed::<32>()? == t,
            "Offer scope/transcript"
        );
        let (mut private, otk) = Native2::fresh(true)?;
        let otk = otk.context("B OTK")?;
        eprintln!("od2_account_constructor context={c} role=1 reserved_one=true");
        let grant = wire::ParsedControl2::sign(
            2,
            &wire::encode(
                &a([
                    b(s.head.context),
                    b(offer.id()),
                    b(t),
                    b(shared_admin::random::<32>()?),
                    b(private.account.curve_key()),
                    b(otk),
                    u(1),
                    u(2),
                ]),
                1024,
            )?,
            &self.device,
        )?;
        let mut chosen = db::RuntimeSelection {
            initial_pair: pair,
            selected_consent,
            offer: offer_raw,
            grant: grant.raw().to_vec(),
            ..Default::default()
        };
        proof(&mut chosen, s.head.context)?;
        let selected = selection(&s, &chosen, None)?;
        let mut target = s.head.clone();
        target.phase = 2;
        target.offer = Some(offer.id());
        target.grant = Some(grant.id());
        target.dialog = Some(selected.d());
        target.epoch = Some(selected.e());
        target.mode_ref = Some(selected.proof().q());
        target.native_generation = target
            .native_generation
            .checked_add(1)
            .context("native generation")?;
        private.reservation = Some((offer.id(), grant.id(), otk, 0));
        let raw = private.seal(&s.profile, &target, &self.encryption)?;
        let hash = wire::hash(b"", &raw);
        self.commit(
            &s,
            db::RuntimeOperation::ReserveGrant {
                context: c,
                fence: s.fence(now()?),
                selection: chosen,
                private: raw,
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        self.cache_native(c, hash, private)?;
        Ok(grant.raw().to_vec())
    }
    pub(super) fn init(&mut self, c: u8, grant_raw: Vec<u8>) -> Result<Vec<u8>> {
        let (s, _) = self.require(c, true)?;
        ensure!(s.head.role == 0 && s.head.phase >= 1, "NeedOffer");
        if s.head.phase == 3 {
            ensure!(s.selection.grant == grant_raw, "CreationConflict");
            return Ok(self.repository.source(c, 0, 0)?.context("A0 missing")?.raw);
        }
        let mut chosen = s.selection.clone();
        chosen.grant = grant_raw;
        proof(&mut chosen, s.head.context)?;
        let selected = selection(&s, &chosen, None)?;
        let grant = wire::ParsedControl2::parse(&chosen.grant)?;
        let g = grant.body().fields(8)?;
        let mut private = self.native(c, &s)?;
        let session = private.account.outbound(g[4].fixed()?, g[5].fixed()?)?;
        eprintln!("od2_session_constructor context={c} role=0");
        let sid = session.sid();
        private.session = Some(session);
        private.guard = Some(Guard2::new(0)?);
        let selected = wire::StaticSelection2::new(selected.proof().clone(), Some(sid.clone()))?;
        let mut target = s.head.clone();
        target.phase = 3;
        target.grant = Some(grant.id());
        target.dialog = Some(selected.d());
        target.epoch = Some(selected.e());
        target.sid = Some(sid);
        target.mode_ref = Some(selected.proof().q());
        target.native_generation = target
            .native_generation
            .checked_add(1)
            .context("native generation")?;
        let original = pair_credentials(&chosen)?;
        let (event, row) = self.trial_send(
            c,
            &s,
            &mut private,
            &selected,
            &chosen,
            data::SendInput2 {
                credential: &original[0],
                sequence: s.profile.next_author_sequence,
                request: [0; 16],
                text: String::new(),
            },
        )?;
        let after = private.seal(&s.profile, &target, &self.encryption)?;
        let hash = wire::hash(b"", &after);
        self.stats.max_native_bytes = self.stats.max_native_bytes.max(after.len());
        self.commit(
            &s,
            db::RuntimeOperation::Genesis {
                context: c,
                fence: s.fence(now()?),
                selection: chosen,
                steps: vec![db::RuntimeGenesisStep {
                    row,
                    native: db::HistoryNativeTransition {
                        before_hash: None,
                        after_blob: after,
                        frame_ids: vec![event.id()],
                    },
                }],
            },
            Purpose2::Peer,
            (1, 0),
        )?;
        self.cache_native(c, hash, private)?;
        Ok(event.raw().to_vec())
    }
    pub(super) fn receive_genesis(&mut self, c: u8, raw: Vec<u8>) -> Result<Vec<u8>> {
        let (s, _) = self.require(c, true)?;
        let event = wire::ParsedEvent2::parse(&raw)?;
        ensure!(
            event.core().n == 0 && event.core().role == 1 - s.head.role,
            "genesis role"
        );
        if let Some(old) = if s.head.phase == 3 {
            self.repository.source(c, event.core().role, 0)?
        } else {
            None
        } {
            ensure!(old.raw == raw, "signed genesis fork");
            return Ok(if s.head.role == 1 {
                self.repository.source(c, 1, 0)?.context("B0")?.raw
            } else {
                vec![]
            });
        }
        let chosen = s.selection.clone();
        let selected = selection(&s, &chosen, Some(event.core().sid.clone()))?;
        let originals = pair_credentials(&chosen)?;
        let peer = usize::from(1 - s.head.role);
        let verified = event.clone().verify(&selected, &originals[peer])?;
        ensure!(
            (s.head.role == 1 && s.head.phase == 2) || (s.head.role == 0 && s.head.phase == 3),
            "genesis phase"
        );
        let mut private = self.native(c, &s)?;
        if private.guard.is_none() {
            private.guard = Some(Guard2::new(s.head.role)?);
        }
        let mut target = s.head.clone();
        target.phase = 3;
        target.sid = Some(event.core().sid.clone());
        target.native_generation = target
            .native_generation
            .checked_add(1)
            .context("native generation")?;
        self.trial_receive(c, &s, &mut private, &selected, &chosen, &verified)?;
        if let Some(r) = private.reservation.as_mut() {
            ensure!(r.3 == 0, "OTK already consumed");
            r.3 = 1;
        }
        let after = private.seal(&s.profile, &target, &self.encryption)?;
        let mut steps = vec![db::RuntimeGenesisStep {
            row: db::HistoryReadable2 {
                source: raw,
                text: String::new(),
                auth: auth(&[&originals[peer]]),
                readable_author_ref: originals[peer].reference(),
                receiver_receipt: None,
            },
            native: db::HistoryNativeTransition {
                before_hash: if s.head.role == 0 {
                    s.head.native_hash
                } else {
                    None
                },
                after_blob: after.clone(),
                frame_ids: vec![event.id()],
            },
        }];
        let output = if s.head.role == 1 {
            let (confirm, row) = self.trial_send(
                c,
                &s,
                &mut private,
                &selected,
                &chosen,
                data::SendInput2 {
                    credential: &originals[1],
                    sequence: s.profile.next_author_sequence,
                    request: [0; 16],
                    text: String::new(),
                },
            )?;
            let final_blob = private.seal(&s.profile, &target, &self.encryption)?;
            self.stats.max_native_bytes = self.stats.max_native_bytes.max(final_blob.len());
            steps.push(db::RuntimeGenesisStep {
                row,
                native: db::HistoryNativeTransition {
                    before_hash: Some(wire::hash(b"", &after)),
                    after_blob: final_blob,
                    frame_ids: vec![confirm.id()],
                },
            });
            confirm.raw().to_vec()
        } else {
            vec![]
        };
        let calls = (u64::from(s.head.role == 1), 1);
        let hash = wire::hash(
            b"",
            &steps.last().context("genesis steps")?.native.after_blob,
        );
        self.commit(
            &s,
            db::RuntimeOperation::Genesis {
                context: c,
                fence: s.fence(now()?),
                selection: chosen,
                steps,
            },
            Purpose2::Peer,
            calls,
        )?;
        self.cache_native(c, hash, private)?;
        Ok(output)
    }
    pub(super) fn ready(&mut self, c: u8) -> Result<Vec<u8>> {
        let (s, current) = self.require(c, true)?;
        if let Some(raw) = &s.head.ready[usize::from(s.head.role)] {
            return Ok(raw.clone());
        }
        let selected = self.selected(&s)?;
        let genesis = [
            self.repository.source(c, 0, 0)?.context("A0")?.id,
            self.repository.source(c, 1, 0)?.context("B0")?.id,
        ];
        let control = wire::ParsedControl2::sign(
            3,
            &wire::encode(
                &a([
                    b(selected.d()),
                    b(selected.e()),
                    b(selected.sid().context("SID")?.as_bytes()),
                    b(genesis[0]),
                    b(genesis[1]),
                    u(u64::from(s.head.role)),
                    u(1),
                    u(2),
                    b(selected.proof().q()),
                ]),
                1024,
            )?,
            &self.device,
        )?;
        control.clone().verify_ready(
            &selected,
            s.head.role,
            genesis,
            &current.peers[usize::from(s.head.role)],
        )?;
        self.commit(
            &s,
            db::RuntimeOperation::Ready {
                context: c,
                fence: s.fence(now()?),
                raw: control.raw().to_vec(),
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        Ok(control.raw().to_vec())
    }
    pub(super) fn accept_ready(&mut self, c: u8, raw: Vec<u8>) -> Result<()> {
        let (s, current) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let genesis = [
            self.repository.source(c, 0, 0)?.context("A0")?.id,
            self.repository.source(c, 1, 0)?.context("B0")?.id,
        ];
        wire::ParsedControl2::parse(&raw)?.verify_ready(
            &selected,
            1 - s.head.role,
            genesis,
            &current.peers[usize::from(1 - s.head.role)],
        )?;
        if s.head.ready[usize::from(1 - s.head.role)].as_ref() == Some(&raw) {
            return Ok(());
        }
        self.commit(
            &s,
            db::RuntimeOperation::Ready {
                context: c,
                fence: s.fence(now()?),
                raw,
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        Ok(())
    }
}
