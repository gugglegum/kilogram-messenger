use super::tests::{TestResult, fixture, id};
use super::*;

#[test]
fn history_verified_cache_mutable_updates_negative_entries_and_stale_versions() -> TestResult {
    let (_dir, store) = fixture()?;
    let tx = store.vault.database.begin_read()?;
    let nodes = tx.open_table(NODES)?;
    let values = tx.open_table(VALUES)?;
    let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
    for key in [
        Key::hash(0, Namespace::Native, [0; 32])?,
        Key::hash(0, Namespace::Current, id(1))?,
        Key::request(0, [1; 16])?,
    ] {
        assert!(w.get(key)?.is_none());
        let absent = std::mem::take(&mut w.verified_values);
        w.put(key, b"one", 0, false)?;
        w.verified_values = absent;
        assert_eq!(w.get(key)?, Some(b"one".to_vec()));
        let stale = std::mem::take(&mut w.verified_values);
        w.put(key, b"two", 0, false)?;
        w.verified_values = stale;
        assert_eq!(w.get(key)?, Some(b"two".to_vec()));
        w.put(key, b"two", 17, true)?;
        assert_eq!(
            w.verified_values
                .stamp(key.slot()? as u8, key.bytes)
                .and_then(|(_, v)| v)
                .map(|v| v.reserved),
            Some(17)
        );
        w.remove(key)?;
        assert!(w.get(key)?.is_none());
        w.put(key, b"new", 0, false)?;
        assert_eq!(w.get(key)?, Some(b"new".to_vec()));
    }
    let one = Key::hash(0, Namespace::Current, id(2))?;
    let two = Key::hash(0, Namespace::Current, id(3))?;
    w.put(one, b"unchanged", 0, false)?;
    w.put(two, b"other root version", 0, false)?;
    assert_eq!(w.get(one)?, Some(b"unchanged".to_vec()));
    Ok(())
}

#[test]
fn history_verified_cache_global_aliases_scope_and_entry_byte_eviction() -> TestResult {
    let (_dir, store) = fixture()?;
    let tx = store.vault.database.begin_read()?;
    let nodes = tx.open_table(NODES)?;
    let values = tx.open_table(VALUES)?;
    let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
    let a = Key::ordinal(0, Namespace::Author0, 17)?;
    let b = Key::ordinal(1, Namespace::Author0, 17)?;
    assert!(w.get(b)?.is_none());
    let mut owner0 = vec![5; 32];
    owner0.extend(id(9));
    w.put(a, &owner0, 0, false)?;
    assert_eq!(w.get(b)?, Some(owner0.clone()));
    let mut owner1 = vec![6; 32];
    owner1.extend(id(10));
    assert!(matches!(w.put(b, &owner1, 0, false), Err(Error::Conflict)));
    w.remove(a)?;
    assert!(w.get(b)?.is_none());
    w.put(b, &owner1, 0, false)?;
    assert_eq!(w.get(a)?, Some(owner1));
    for c in 0..2 {
        w.put(Key::hash(c, Namespace::Current, id(18))?, &[c], 0, false)?;
    }
    assert_eq!(
        w.get(Key::hash(0, Namespace::Current, id(18))?)?,
        Some(vec![0])
    );
    assert_eq!(
        w.get(Key::hash(1, Namespace::Current, id(18))?)?,
        Some(vec![1])
    );
    for n in 100..500 {
        w.get(Key::hash(0, Namespace::Control, id(n))?)?;
        let (entries, bytes) = w.verified_values.sizes();
        assert!(entries <= verified_values::ENTRY_CAP && bytes <= verified_values::BYTE_CAP);
    }
    for n in 0..4 {
        let key = Key::hash(0, Namespace::Current, id(1000 + n))?;
        w.put(key, &vec![n as u8; MAX_VALUE], 0, false)?;
        let (entries, bytes) = w.verified_values.sizes();
        assert!(entries <= 128 && bytes <= 1024 * 1024);
    }
    assert_eq!(
        w.get(Key::hash(0, Namespace::Current, id(1000))?)?,
        Some(vec![0; MAX_VALUE])
    );
    w.root.incarnation[0] ^= 1;
    assert!(matches!(w.get(a), Err(Error::Integrity(_))));
    w.root.incarnation[0] ^= 1;
    w.root.contexts[0][0] ^= 1;
    assert!(matches!(w.get(a), Err(Error::Integrity(_))));
    Ok(())
}

#[test]
fn history_verified_cache_never_crosses_transactions_or_hides_new_access_corruption() -> TestResult
{
    let (_dir, mut store) = fixture()?;
    let key = Key::hash(0, Namespace::Current, id(700))?;
    store.commit(
        store.generation(),
        id(701),
        vec![Mutation::put(key, b"verified".to_vec())],
        vec![],
    )?;
    assert_eq!(store.get(key)?, Some(b"verified".to_vec()));
    let address = store
        .crypto
        .address(key.slot()? as u8, b"value", &key.bytes);
    let tx = store.vault.database.begin_write()?;
    {
        let mut table = tx.open_table(VALUES)?;
        let mut raw = table
            .get(address.as_slice())?
            .ok_or("cipher")?
            .value()
            .to_vec();
        let last = raw.len() - 1;
        raw[last] ^= 1;
        table.insert(address.as_slice(), raw.as_slice())?;
    }
    tx.commit()?;
    assert!(matches!(store.get(key), Err(Error::Integrity(_))));
    Ok(())
}
