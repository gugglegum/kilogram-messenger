//! Structural differential vectors; no Root/MAC/current-policy claims.
use super::*;
use serde_json::{Value as J, json};
type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|x| format!("{x:02x}")).collect()
}
pub(super) fn unhex(raw: &str) -> TestResult<Vec<u8>> {
    if !raw.len().is_multiple_of(2) {
        return Err("odd hex".into());
    }
    (0..raw.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&raw[i..i + 2], 16)?))
        .collect()
}
fn optional(id: Option<Id>) -> J {
    id.map_or(J::Null, |id| json!(hex(&id)))
}
#[test]
fn a3_s3_differential_37_state_reader() -> TestResult {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../kilogram-protocol/tests/data/s3-differential.json"
    );
    let vectors: Vec<J> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    assert_eq!(vectors.len(), 37);
    let mut successes = 0;
    let mut rejects = 0;
    for v in vectors {
        let raw = unhex(v["raw"].as_str().ok_or("raw")?)?;
        let parsed: Result<J> = match v["reader"].as_str().ok_or("reader")? {
            "event" => (|| {
                let e = read_event2(&raw)?;
                let core = &raw[usize::from(e.core_offset)
                    ..usize::from(e.core_offset) + usize::from(e.core_len)];
                Ok(
                    json!({"c":hex(&e.scope.context),"d":hex(&e.scope.dialog),"e":hex(&e.scope.epoch),"sid":e.scope.session_id,
                    "role":e.role,"n":e.n,"previous":optional(e.previous),"sequence":e.sequence,"kind":e.kind,
                    "request":hex(&e.request),"parents":e.parents.iter().map(|id|hex(id)).collect::<Vec<_>>(),
                    "g":e.g,"j":e.index,"parent":optional(e.parent),"tail":e.tail.map_or(J::Null,|t|json!([t.g,hex(&t.key),t.index,hex(&t.last)])),
                    "author":hex(&e.author),"q":hex(&e.q),"key":hex(&e.key),"native_kind":e.native_kind,
                    "core_span":[e.core_offset,e.core_len],"core_raw":hex(core),"id":hex(&e.id)}),
                )
            })(),
            "receipt" => (|| {
                let r = read_receipt2(&raw)?;
                Ok(
                    json!({"c":hex(&r.scope.context),"d":hex(&r.scope.dialog),"e":hex(&r.scope.epoch),"sid":r.scope.session_id,
                "q":hex(&r.q),"format":r.format,"role":r.role,"n":r.n,"event":hex(&r.event),"cert":hex(&r.cert),"auth":hex(&r.auth),
                "id":hex(&hash_parts(b"kg:od2:receipt-id\0",&[&raw]))}),
                )
            })(),
            "proof" => (|| {
                let p = read_fresh_binding2(&raw)?;
                Ok(
                    json!({"c":hex(&p.c),"certs":p.certs.iter().map(|id|hex(id)).collect::<Vec<_>>(),
                "q":hex(&p.q),"d":hex(&p.d),"e":hex(&p.e),"t":hex(&p.transcript),"offer":hex(&p.offer),"grant":hex(&p.grant)}),
                )
            })(),
            "auth" => (|| {
                let a = read_auth_bundle(&raw)?;
                Ok(json!({"id":hex(&a.id),"cert":hex(&a.cert)}))
            })(),
            _ => return Err("reader".into()),
        };
        if v["expected"].is_null() {
            assert!(parsed.is_err(), "{} accepted malformed", v["name"]);
            rejects += 1;
        } else {
            assert_eq!(parsed?, v["expected"], "{}", v["name"]);
            successes += 1;
        }
    }
    assert_eq!((successes, rejects), (19, 18));
    println!(
        "a3_s3_D01_D09_state=37 field_comparisons={successes} matching_rejects={rejects} native_calls=0"
    );
    Ok(())
}
