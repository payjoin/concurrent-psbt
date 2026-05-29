use crate::field;

/// PSBT magic bytes: "psbt\xff"
const MAGIC: [u8; 5] = [0x70, 0x73, 0x62, 0x74, 0xff];

/// Errors that can occur while scrubbing a PSBT.
#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidMagic,
    UnexpectedEof,
    VersionFieldNotFound,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidMagic => write!(f, "invalid PSBT magic bytes"),
            Error::UnexpectedEof => write!(f, "unexpected end of input"),
            Error::VersionFieldNotFound => write!(f, "version field not found"),
        }
    }
}

impl std::error::Error for Error {}

/// Scrub a PSBT, retaining only non-sensitive fields safe to share across a
/// trust boundary.
///
/// Reads `psbt`, copies only fields listed in [`crate::field`] into a fresh
/// output PSBT.
/// 
pub fn scrub(psbt: &[u8]) -> Result<Vec<u8>, Error> {
    let mut pos = 0;

    if psbt.get(..5) != Some(&MAGIC) {
        return Err(Error::InvalidMagic);
    }
    pos += 5;

    let global = parse_map(psbt, &mut pos)?;

    let version = global
        .iter()
        .find(|(k, _)| k.as_slice() == [field::global::VERSION])
        .and_then(|(_, v)| v.first().copied())
        .ok_or(Error::VersionFieldNotFound)?;

    let (input_count, output_count) = if version == 2 {
        let ic = global
            .iter()
            .find(|(k, _)| k.as_slice() == [field::global::INPUT_COUNT])
            .ok_or(Error::UnexpectedEof)
            .and_then(|(_, v)| compact_size_from_slice(v))?;
        let oc = global
            .iter()
            .find(|(k, _)| k.as_slice() == [field::global::OUTPUT_COUNT])
            .ok_or(Error::UnexpectedEof)
            .and_then(|(_, v)| compact_size_from_slice(v))?;
        (ic, oc)
    } else {
        let (_, tx) = global
            .iter()
            .find(|(k, _)| k.as_slice() == [field::global::UNSIGNED_TX])
            .ok_or(Error::UnexpectedEof)?;
        tx_counts(tx)?
    };

    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    write_filtered_map(&mut out, &global, global_keep);
    for _ in 0..input_count {
        let map = parse_map(psbt, &mut pos)?;
        write_filtered_map(&mut out, &map, input_keep);
    }
    for _ in 0..output_count {
        let map = parse_map(psbt, &mut pos)?;
        write_filtered_map(&mut out, &map, output_keep);
    }

    Ok(out)
}

// TODO: not fan of using matches!, there are footguns. Best to use a const array in fields and use rust-psbt crate instead of duplicating the u8's

fn global_keep(t: u8) -> bool {
    matches!(
        t,
        field::global::UNSIGNED_TX
            | field::global::TX_VERSION
            | field::global::FALLBACK_LOCKTIME
            | field::global::INPUT_COUNT
            | field::global::OUTPUT_COUNT
            | field::global::TX_MODIFIABLE
            | field::global::VERSION
    )
}

fn input_keep(t: u8) -> bool {
    matches!(
        t,
        field::input::NON_WITNESS_UTXO
            | field::input::WITNESS_UTXO
            | field::input::SIGHASH_TYPE
            | field::input::REDEEM_SCRIPT
            | field::input::WITNESS_SCRIPT
            | field::input::FINAL_SCRIPTSIG
            | field::input::FINAL_SCRIPTWITNESS
            | field::input::PREVIOUS_TXID
            | field::input::OUTPUT_INDEX
            | field::input::SEQUENCE
            | field::input::REQUIRED_TIME_LOCKTIME
            | field::input::REQUIRED_HEIGHT_LOCKTIME
            | field::input::TAP_KEY_SIG
            | field::input::TAP_SCRIPT_SIG
            | field::input::TAP_LEAF_SCRIPT
    )
}

fn output_keep(t: u8) -> bool {
    matches!(t, field::output::AMOUNT | field::output::SCRIPT)
}

fn parse_map(data: &[u8], pos: &mut usize) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Error> {
    let mut entries = Vec::new();
    loop {
        let key_len = read_compact_size(data, pos)?;
        if key_len == 0 {
            break;
        }
        let key = read_bytes(data, pos, key_len as usize)?;
        let val_len = read_compact_size(data, pos)?;
        let val = read_bytes(data, pos, val_len as usize)?;
        entries.push((key, val));
    }
    Ok(entries)
}

fn write_filtered_map(out: &mut Vec<u8>, entries: &[(Vec<u8>, Vec<u8>)], keep: fn(u8) -> bool) {
    for (key, val) in entries {
        if key.first().copied().map(keep).unwrap_or(false) {
            write_compact_size(out, key.len() as u64);
            out.extend_from_slice(key);
            write_compact_size(out, val.len() as u64);
            out.extend_from_slice(val);
        }
    }
    out.push(0x00);
}

fn read_compact_size(data: &[u8], pos: &mut usize) -> Result<u64, Error> {
    let first = *data.get(*pos).ok_or(Error::UnexpectedEof)?;
    *pos += 1;
    match first {
        n @ 0x00..=0xfc => Ok(n as u64),
        0xfd => {
            let b: [u8; 2] = data
                .get(*pos..*pos + 2)
                .ok_or(Error::UnexpectedEof)?
                .try_into()
                .unwrap();
            *pos += 2;
            Ok(u16::from_le_bytes(b) as u64)
        }
        0xfe => {
            let b: [u8; 4] = data
                .get(*pos..*pos + 4)
                .ok_or(Error::UnexpectedEof)?
                .try_into()
                .unwrap();
            *pos += 4;
            Ok(u32::from_le_bytes(b) as u64)
        }
        0xff => {
            let b: [u8; 8] = data
                .get(*pos..*pos + 8)
                .ok_or(Error::UnexpectedEof)?
                .try_into()
                .unwrap();
            *pos += 8;
            Ok(u64::from_le_bytes(b))
        }
    }
}

fn write_compact_size(out: &mut Vec<u8>, n: u64) {
    if n < 0xfd {
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else if n <= 0xffff_ffff {
        out.push(0xfe);
        out.extend_from_slice(&(n as u32).to_le_bytes());
    } else {
        out.push(0xff);
        out.extend_from_slice(&n.to_le_bytes());
    }
}

fn read_bytes(data: &[u8], pos: &mut usize, len: usize) -> Result<Vec<u8>, Error> {
    let slice = data.get(*pos..*pos + len).ok_or(Error::UnexpectedEof)?;
    *pos += len;
    Ok(slice.to_vec())
}

fn compact_size_from_slice(data: &[u8]) -> Result<u64, Error> {
    read_compact_size(data, &mut 0)
}

fn skip(data: &[u8], pos: &mut usize, n: usize) -> Result<(), Error> {
    if *pos + n > data.len() {
        Err(Error::UnexpectedEof)
    } else {
        *pos += n;
        Ok(())
    }
}

/// Extract `(input_count, output_count)` from a non-witness-serialised
/// unsigned Bitcoin transaction (as stored in `PSBT_GLOBAL_UNSIGNED_TX`).
fn tx_counts(tx: &[u8]) -> Result<(u64, u64), Error> {
    let mut pos = 0;
    skip(tx, &mut pos, 4)?; // version
    let input_count = read_compact_size(tx, &mut pos)?;
    for _ in 0..input_count {
        skip(tx, &mut pos, 36)?; // outpoint (txid + vout)
        let script_len = read_compact_size(tx, &mut pos)?;
        skip(tx, &mut pos, script_len as usize)?; // scriptSig (empty in unsigned tx)
        skip(tx, &mut pos, 4)?; // sequence
    }
    let output_count = read_compact_size(tx, &mut pos)?;
    // Just need the count, return early.
    Ok((input_count, output_count))
}

#[cfg(any(test, feature = "unit-tests"))]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::field;

    // Build a raw key-value entry for a PSBT map.
    fn kv(key: &[u8], val: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        write_compact_size(&mut buf, key.len() as u64);
        buf.extend_from_slice(key);
        write_compact_size(&mut buf, val.len() as u64);
        buf.extend_from_slice(val);
        buf
    }

    // Build a minimal v2 global map with VERSION, TX_VERSION, INPUT_COUNT,
    // OUTPUT_COUNT, and any extra entries, followed by the map terminator.
    fn v2_global(input_count: u8, output_count: u8, extra: &[Vec<u8>]) -> Vec<u8> {
        let mut map = Vec::new();
        map.extend(kv(&[field::global::VERSION], &[2, 0, 0, 0]));
        map.extend(kv(&[field::global::TX_VERSION], &[2, 0, 0, 0]));
        map.extend(kv(&[field::global::INPUT_COUNT], &[input_count]));
        map.extend(kv(&[field::global::OUTPUT_COUNT], &[output_count]));
        for e in extra {
            map.extend(e);
        }
        map.push(0x00);
        map
    }

    fn empty_map() -> Vec<u8> {
        vec![0x00]
    }

    fn v2_psbt(
        input_count: u8,
        output_count: u8,
        global_extra: &[Vec<u8>],
        maps: &[Vec<u8>],
    ) -> Vec<u8> {
        let mut buf = MAGIC.to_vec();
        buf.extend(v2_global(input_count, output_count, global_extra));
        for m in maps {
            buf.extend(m);
        }
        buf
    }

    // A minimal non-witness unsigned transaction with the given input and output
    // counts (empty scriptSigs and scriptPubKeys — not valid Bitcoin, fine for parsing).
    fn dummy_tx(input_count: u8, output_count: u8) -> Vec<u8> {
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes()); // version = 1
        tx.push(input_count);
        for _ in 0..input_count {
            tx.extend_from_slice(&[0u8; 32]); // txid
            tx.extend_from_slice(&0u32.to_le_bytes()); // vout
            tx.push(0x00); // scriptSig len = 0
            tx.extend_from_slice(&u32::MAX.to_le_bytes()); // sequence
        }
        tx.push(output_count);
        for _ in 0..output_count {
            tx.extend_from_slice(&1000u64.to_le_bytes()); // value
            tx.push(0x00); // scriptPubKey len = 0
        }
        tx.extend_from_slice(&0u32.to_le_bytes()); // locktime
        tx
    }

    #[test]
    fn scrub_empty_v2_roundtrip() {
        // A v2 PSBT with only insensitive global fields and no inputs/outputs
        // should survive scrubbing unchanged.
        let psbt = v2_psbt(0, 0, &[], &[]);
        assert_eq!(scrub(&psbt).unwrap(), psbt);
    }

    #[test]
    fn scrub_strips_xpub() {
        let xpub = kv(&[0x01], &[0xDE, 0xAD]); // GLOBAL_XPUB — sensitive
        let psbt = v2_psbt(0, 0, &[xpub], &[]);
        let result = scrub(&psbt).unwrap();
        assert_eq!(result, v2_psbt(0, 0, &[], &[]));
    }

    #[test]
    fn scrub_strips_bip32_derivation_in_input() {
        let witness_utxo = kv(&[field::input::WITNESS_UTXO], &[0xAA, 0xBB]);
        let bip32 = kv(&[0x06, 0x02, 0x03], &[0xCC]); // INPUT_BIP32_DERIVATION — sensitive
        let mut input_map = Vec::new();
        input_map.extend(&witness_utxo);
        input_map.extend(&bip32);
        input_map.push(0x00);

        let psbt = v2_psbt(1, 0, &[], &[input_map]);
        let result = scrub(&psbt).unwrap();

        let mut expected_input = Vec::new();
        expected_input.extend(&witness_utxo);
        expected_input.push(0x00);
        let expected = v2_psbt(1, 0, &[], &[expected_input]);

        assert_eq!(result, expected);
    }

    #[test]
    fn scrub_strips_output_redeem_script() {
        let amount = kv(
            &[field::output::AMOUNT],
            &[0xE8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );
        let script = kv(&[field::output::SCRIPT], &[0x51]); // OP_1
        let redeem = kv(&[0x00], &[0x52]); // OUTPUT_REDEEM_SCRIPT — sensitive

        let mut output_map = Vec::new();
        output_map.extend(&amount);
        output_map.extend(&script);
        output_map.extend(&redeem);
        output_map.push(0x00);

        let psbt = v2_psbt(0, 1, &[], &[output_map]);
        let result = scrub(&psbt).unwrap();

        let mut expected_output = Vec::new();
        expected_output.extend(&amount);
        expected_output.extend(&script);
        expected_output.push(0x00);
        let expected = v2_psbt(0, 1, &[], &[expected_output]);

        assert_eq!(result, expected);
    }

    #[test]
    fn scrub_strips_proprietary() {
        // 0xFC is proprietary — sensitive in global, input, and output maps.
        let prop = kv(&[0xFC, 0x01], &[0xFF]);

        let mut input_map = prop.clone();
        input_map.push(0x00);
        let mut output_map = prop.clone();
        output_map.push(0x00);

        let psbt = v2_psbt(1, 1, &[prop], &[input_map, output_map]);
        let result = scrub(&psbt).unwrap();

        // All three maps should now be empty (only terminators).
        let expected = v2_psbt(1, 1, &[], &[empty_map(), empty_map()]);
        assert_eq!(result, expected);
    }

    #[test]
    fn scrub_v0() {
        let tx = dummy_tx(1, 1);
        let unsigned_tx = kv(&[field::global::UNSIGNED_TX], &tx);
        let xpub = kv(&[0x01], &[0xDE, 0xAD]); // sensitive

        let mut global = Vec::new();
        global.extend(&unsigned_tx);
        global.extend(&xpub);
        global.push(0x00);

        let witness_utxo = kv(&[field::input::WITNESS_UTXO], &[0xAA]);
        let bip32 = kv(&[0x06, 0x02], &[0xBB]); // INPUT_BIP32_DERIVATION — sensitive
        let mut input_map = Vec::new();
        input_map.extend(&witness_utxo);
        input_map.extend(&bip32);
        input_map.push(0x00);

        // v0 output: only sensitive fields (BIP32_DERIVATION = 0x02)
        let out_bip32 = kv(&[0x02], &[0xCC]); // OUTPUT_BIP32_DERIVATION — sensitive
        let mut output_map = out_bip32;
        output_map.push(0x00);

        let mut psbt = MAGIC.to_vec();
        psbt.extend(&global);
        psbt.extend(&input_map);
        psbt.extend(&output_map);

        let result = scrub(&psbt).unwrap();

        // Expected: UNSIGNED_TX kept, XPUB gone; WITNESS_UTXO kept, BIP32 gone; output empty.
        let mut expected_global = Vec::new();
        expected_global.extend(&unsigned_tx);
        expected_global.push(0x00);
        let mut expected_input = Vec::new();
        expected_input.extend(&witness_utxo);
        expected_input.push(0x00);

        let mut expected = MAGIC.to_vec();
        expected.extend(&expected_global);
        expected.extend(&expected_input);
        expected.extend(empty_map());

        assert_eq!(result, expected);
    }

    #[test]
    fn invalid_magic() {
        assert_eq!(scrub(b"not a psbt"), Err(Error::InvalidMagic));
    }

    #[test]
    fn unexpected_eof_truncated_after_magic() {
        assert_eq!(scrub(&MAGIC), Err(Error::UnexpectedEof));
    }

    #[test]
    fn unexpected_eof_truncated_mid_map() {
        // Magic + start of a key (length byte says 5 but no data follows)
        let mut psbt = MAGIC.to_vec();
        psbt.push(0x05); // key length = 5, but no key bytes follow
        assert_eq!(scrub(&psbt), Err(Error::UnexpectedEof));
    }
}
