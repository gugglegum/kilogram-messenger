//! Shared exact raw vectors and construction oracles, not native/Root authorization evidence.
use super::*;
use serde_json::{Value as J, json};
use support::{hex, unhex};
const GOLDENS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/s3-differential.json"
);
fn hx(id: &Id) -> String {
    hex(id)
}
fn optional(id: Option<Id>) -> J {
    id.map_or(J::Null, |id| json!(hex(&id)))
}
fn tail(t: Option<&ClosedTail>) -> J {
    t.map_or(J::Null, |t| json!([t.g, hx(&t.key), t.index, hx(&t.last)]))
}
fn event_projection(core: &Core2, key: Id, native_kind: u8, raw: &[u8]) -> TestResult<J> {
    let core_raw = encode(&core.value(), MAX_CORE)?;
    Ok(
        json!({"c":hx(&core.c),"d":hx(&core.d),"e":hx(&core.e),"sid":core.sid,
        "role":core.role,"n":core.n,"previous":optional(core.previous),"sequence":core.sequence,"kind":core.kind,
        "request":hex(&core.request),"parents":core.parents.iter().map(hx).collect::<Vec<_>>(),
        "g":core.expected.g,"j":core.expected.index,"parent":optional(core.expected.parent),"tail":tail(core.expected.tail.as_ref()),
        "author":hx(&core.author_auth),"q":hx(&core.q),"key":hx(&key),"native_kind":native_kind,
        "core_span":[2,core_raw.len()],"core_raw":hex(&core_raw),"id":hx(&hash(b"kg:od2:event-id\0",raw))}),
    )
}
fn receipt_projection(f: &ReceiptFields2, raw: &[u8]) -> J {
    json!({"c":hx(&f.c),"d":hx(&f.d),"e":hx(&f.e),"sid":f.sid,"q":hx(&f.q),"format":f.format,
        "role":f.lane,"n":f.n,"event":hx(&f.event),"cert":hx(&f.receiver_cert),"auth":hx(&f.receiver_auth),
        "id":hx(&hash(b"kg:od2:receipt-id\0",raw))})
}
fn put(out: &mut Vec<J>, name: &str, reader: &str, raw: Vec<u8>, expected: Option<J>) {
    out.push(json!({"name":name,"reader":reader,"raw":hex(&raw),"expected":expected}));
}
fn values(v: &Value) -> TestResult<Vec<Value>> {
    Ok(v.list()?.to_vec())
}
fn raw_event(c: &Core2, key: Id) -> TestResult<Vec<u8>> {
    Ok(encode(
        &a([
            u(2),
            c.value(),
            a([
                u(if c.kind == 1 { 0 } else { 1 }),
                b(key),
                u(c.expected.index),
            ]),
            b([0x44]),
            b([0; 64]),
        ]),
        MAX_EVENT,
    )?)
}
fn fresh_projection(actors: &support::Actors, f: &support::Fresh) -> TestResult<J> {
    let t = f.offer.body().fields(8)?[3].fixed::<32>()?;
    let e = hash(
        b"kg:od2:epoch\0",
        &encode(
            &a([
                u(2),
                b(actors.c),
                b(actors.credentials[0].cert_ref()),
                b(actors.credentials[1].cert_ref()),
                b(f.offer.raw()),
                b(f.grant.raw()),
            ]),
            4096,
        )?,
    );
    let d = hash(
        b"kg:od2:domain\0",
        &encode(
            &a([
                u(2),
                u(1),
                u(2),
                b(actors.c),
                b(actors.credentials[0].cert_ref()),
                b(actors.credentials[1].cert_ref()),
                b(e),
            ]),
            256,
        )?,
    );
    Ok(
        json!({"c":hx(&actors.c),"certs":actors.credentials.iter().map(|c|hx(&c.cert_ref())).collect::<Vec<_>>(),
        "t":hx(&t),"offer":hx(&hash(b"kg:od2:control-id\0",f.offer.raw())),"grant":hx(&hash(b"kg:od2:control-id\0",f.grant.raw())),
        "q":hx(&hash(b"kg:od2:mode-certificate\0",&f.raw)),"d":hx(&d),"e":hx(&e)}),
    )
}
fn construction_vectors() -> TestResult<Vec<J>> {
    let actors = support::Actors::new([0xd3; 32])?;
    let f = actors.fresh([61; 32], [62; 32], [63; 32])?;
    let mut out = vec![];
    let core = |role: u8, n: u64| Core2 {
        c: actors.c,
        d: f.proof.proof().d(),
        e: f.proof.proof().e(),
        sid: "S".repeat(43),
        role,
        n,
        previous: (n > 0).then_some([12; 32]),
        sequence: n,
        kind: if n == 0 { role + 1 } else { 3 },
        request: if n == 0 {
            let id = if role == 0 {
                f.offer.id()
            } else {
                f.grant.id()
            };
            let mut r = [0; 16];
            r.copy_from_slice(&id[..16]);
            r
        } else {
            [role + 11; 16]
        },
        parents: if n > 1 {
            vec![[12; 32], [15; 32]]
        } else {
            vec![]
        },
        expected: Expected {
            g: if role == 0 && n > 0 { 1 } else { 0 },
            index: n.saturating_sub(1),
            parent: (role == 1 || n > 0).then_some([25; 32]),
            tail: None,
        },
        author_auth: actors.credentials[usize::from(role)].reference(),
        q: f.proof.q(),
    };
    for (name, role, n) in [
        ("D01-init", 0, 0),
        ("D01-confirm", 1, 0),
        ("D01-text", 0, 1),
        ("D01-text512", 0, 512),
    ] {
        let c = core(role, n);
        let raw = raw_event(&c, [35 + role; 32])?;
        let expected =
            event_projection(&c, [35 + role; 32], if c.kind == 1 { 0 } else { 1 }, &raw)?;
        put(&mut out, name, "event", raw, Some(expected));
    }
    let mut receipts = vec![];
    for role in 0..2 {
        let c = core(role, 1);
        let e = raw_event(&c, [35 + role; 32])?;
        let fields = ReceiptFields2 {
            c: c.c,
            d: c.d,
            e: c.e,
            sid: c.sid,
            q: c.q,
            format: 2,
            lane: role,
            n: 1,
            event: hash(EVENT_ID, &e),
            receiver_cert: actors.credentials[usize::from(1 - role)].cert_ref(),
            receiver_auth: actors.credentials[usize::from(1 - role)].reference(),
        };
        let raw = encode(&a([fields.value(), b([0; 64])]), MAX_RECEIPT)?;
        put(
            &mut out,
            &format!("D02-lane{role}"),
            "receipt",
            raw.clone(),
            Some(receipt_projection(&fields, &raw)),
        );
        receipts.push(raw);
    }
    put(
        &mut out,
        "D03-fresh",
        "proof",
        f.raw.clone(),
        Some(fresh_projection(&actors, &f)?),
    );
    for (name, credential) in [
        ("D04-O", actors.credentials[0].clone()),
        ("D04-A", actors.renewal(0, 3)?),
        ("D04-F", actors.renewal(0, 4)?),
        ("D04-R", actors.renewal(1, 3)?),
    ] {
        put(
            &mut out,
            name,
            "auth",
            credential.raw().to_vec(),
            Some(
                json!({"id":hx(&hash(b"",credential.raw())),"cert":hx(&hash(b"",&credential.certificate().encode()?))}),
            ),
        );
    }
    let mut variants = vec![];
    let c = core(0, 4);
    let mut v = c.clone();
    v.role = 1;
    variants.push(("role", v, [35; 32]));
    let mut v = c.clone();
    v.n += 1;
    variants.push(("ordinal", v, [35; 32]));
    let mut v = c.clone();
    v.previous = Some([13; 32]);
    v.parents[0] = [13; 32];
    variants.push(("previous", v, [35; 32]));
    let mut v = c.clone();
    v.c = [99; 32];
    variants.push(("context", v, [35; 32]));
    let mut v = c.clone();
    v.q = [99; 32];
    variants.push(("Q", v, [35; 32]));
    let mut v = c.clone();
    v.request = [99; 16];
    variants.push(("FIFO-request", v, [35; 32]));
    variants.push(("seen-key", c.clone(), [34; 32]));
    let mut v = c;
    v.expected.g += 1;
    variants.push(("generation", v, [35; 32]));
    for (name, c, key) in variants {
        let raw = raw_event(&c, key)?;
        let expected = event_projection(&c, key, 1, &raw)?;
        put(
            &mut out,
            &format!("D05-{name}"),
            "event",
            raw,
            Some(expected),
        );
    }
    let e = raw_event(&core(0, 1), [35; 32])?;
    let mut raw = e.clone();
    raw.splice(0..1, [0x98, 5]);
    put(&mut out, "D06-array-overlong", "event", raw, None);
    let mut raw = e.clone();
    raw.splice(3..4, [0x18, 2]);
    put(&mut out, "D06-core-overlong", "event", raw, None);
    let mut raw = e.clone();
    raw.push(0);
    put(&mut out, "D06-trailing", "event", raw, None);
    let mut v = values(&decode(&e, MAX_EVENT)?)?;
    let mut c = values(&v[1])?;
    c.pop();
    v[1] = a(c);
    put(
        &mut out,
        "D06-core-arity",
        "event",
        encode(&a(v), MAX_EVENT)?,
        None,
    );
    let mut v = values(&decode(&e, MAX_EVENT)?)?;
    v[2] = a([u(1), b([35; 32])]);
    put(
        &mut out,
        "D06-header-arity",
        "event",
        encode(&a(v), MAX_EVENT)?,
        None,
    );
    put(&mut out, "D06-cap", "event", vec![0; MAX_EVENT + 1], None);
    let r = &receipts[0];
    let v = decode(r, MAX_RECEIPT)?;
    let body = v.fields(2)?[0].fields(12)?;
    let prefix = encode(&a(body[..8].iter().cloned()), MAX_RECEIPT)?;
    let n_offset = 1 + prefix.len(); // outer array byte + body prefix (same single-byte arity encoding).
    let mut raw = r.clone();
    raw.splice(n_offset..n_offset + 1, [0x18, 1]);
    put(&mut out, "D07-n-overlong", "receipt", raw, None);
    put(
        &mut out,
        "D07-signature63",
        "receipt",
        encode(&a([v.fields(2)?[0].clone(), b([0; 63])]), MAX_RECEIPT)?,
        None,
    );
    put(
        &mut out,
        "D07-body11",
        "receipt",
        encode(&a([a(body[..11].iter().cloned()), b([0; 64])]), MAX_RECEIPT)?,
        None,
    );
    put(
        &mut out,
        "D07-outer3",
        "receipt",
        encode(&a([v.fields(2)?[0].clone(), b([0; 64]), u(0)]), MAX_RECEIPT)?,
        None,
    );
    let proof = values(&decode(&f.raw, MAX_PROOF)?)?;
    put(
        &mut out,
        "D08-outer5",
        "proof",
        encode(&a(proof[..5].iter().cloned()), MAX_PROOF)?,
        None,
    );
    let mut p = proof.clone();
    let mut cons = values(&decode(&f.consent, 4096)?)?;
    let mut cb = values(&cons[0])?;
    cb[6] = a([u(0), u(0)]);
    cons[0] = a(cb);
    p[3] = b(encode(&a(cons), 4096)?);
    put(
        &mut out,
        "D08-origin",
        "proof",
        encode(&a(p), MAX_PROOF)?,
        None,
    );
    let mut p = proof.clone();
    let mut o = values(&decode(f.offer.raw(), 4096)?)?;
    o[2] = a(o[2].list()?[..7].iter().cloned());
    p[4] = b(encode(&a(o), 4096)?);
    put(
        &mut out,
        "D08-offer7",
        "proof",
        encode(&a(p), MAX_PROOF)?,
        None,
    );
    let mut p = proof;
    let mut g = values(&decode(f.grant.raw(), 4096)?)?;
    let mut gb = values(&g[2])?;
    gb[5] = b([63; 31]);
    g[2] = a(gb);
    p[5] = b(encode(&a(g), 4096)?);
    put(
        &mut out,
        "D08-otk31",
        "proof",
        encode(&a(p), MAX_PROOF)?,
        None,
    );
    let auth = actors.credentials[0].raw();
    let av = decode(auth, MAX_AUTH)?;
    let mut raw = auth.to_vec();
    raw.splice(0..1, [0x98, 3]);
    put(&mut out, "D09-overlong", "auth", raw, None);
    put(
        &mut out,
        "D09-arity2",
        "auth",
        encode(&a(av.list()?[..2].iter().cloned()), MAX_AUTH)?,
        None,
    );
    put(
        &mut out,
        "D09-certificate-uint",
        "auth",
        encode(&a([u(1), u(3), av.list()?[2].clone()]), MAX_AUTH)?,
        None,
    );
    put(
        &mut out,
        "D09-authority-array",
        "auth",
        encode(&a([u(1), av.list()?[1].clone(), a([])]), MAX_AUTH)?,
        None,
    );
    assert_eq!(out.len(), 37);
    Ok(out)
}
#[test]
fn a3_s3_differential_37_codec_and_construction() -> TestResult {
    let vectors = construction_vectors()?;
    if std::env::var("KILOGRAM_S3_UPDATE_VECTORS").as_deref() == Ok("1") {
        std::fs::create_dir_all(std::path::Path::new(GOLDENS).parent().ok_or("parent")?)?;
        std::fs::write(
            GOLDENS,
            format!("{}\n", serde_json::to_string_pretty(&vectors)?),
        )?;
    }
    let saved: Vec<J> = serde_json::from_str(&std::fs::read_to_string(GOLDENS)?)?;
    assert_eq!(
        saved, vectors,
        "raw fixtures and construction oracles drifted"
    );
    let mut success = 0;
    let mut reject = 0;
    for v in saved {
        let raw = unhex(v["raw"].as_str().ok_or("raw")?)?;
        let parsed: TestResult<J> = match v["reader"].as_str().ok_or("reader")? {
            "event" => (|| {
                let e = ParsedEvent2::parse(&raw)?;
                let mut projected = event_projection(e.core(), e.key, e.kind, &raw)?;
                let (offset, len) = e.core_span();
                projected["id"] = json!(hx(&e.id()));
                projected["core_span"] = json!([offset, len]);
                projected["core_raw"] =
                    json!(hex(raw.get(offset..offset + len).ok_or("Core span")?));
                Ok(projected)
            })(),
            "receipt" => (|| {
                let r = ParsedReceipt2::parse(&raw)?;
                let mut projected = receipt_projection(r.fields(), &raw);
                projected["id"] = json!(hx(&r.id()));
                Ok(projected)
            })(),
            "proof" => (|| {
                let p = ParsedModeProof2::parse(&raw)?;
                let top = decode(&raw, MAX_PROOF)?;
                let o = ParsedControl2::parse(top.fields(6)?[4].bytes()?)?;
                Ok(
                    json!({"c":hx(&p.c()),"certs":p.certs().iter().map(hx).collect::<Vec<_>>(),"q":hx(&p.q()),"d":hx(&p.d()),"e":hx(&p.e()),
                    "t":hx(&o.body().fields(8)?[3].fixed()?),"offer":p.offer_id().map(|id|hx(&id)),"grant":p.grant_id().map(|id|hx(&id))}),
                )
            })(),
            "auth" => (|| {
                let a = ParsedCredentialBundle::parse(&raw)?;
                Ok(json!({"id":hx(&a.reference()),"cert":hx(&a.cert_ref())}))
            })(),
            _ => return Err("reader".into()),
        };
        if v["expected"].is_null() {
            assert!(parsed.is_err(), "{} accepted malformed", v["name"]);
            reject += 1;
        } else {
            assert_eq!(parsed?, v["expected"], "{}", v["name"]);
            success += 1;
        }
    }
    assert_eq!((success, reject), (19, 18));
    println!(
        "a3_s3_D01_D09_codec=37 field_comparisons={success} matching_rejects={reject} native_calls=0"
    );
    Ok(())
}
