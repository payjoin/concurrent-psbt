/// Non-sensitive PSBT field key types that the scrubber passes through.
///
/// Only insensitive fields are listed here. These are fields that are
/// safe to share with an untrusted peer

pub mod global {
    pub const UNSIGNED_TX: u8 = 0x00;
    pub const TX_VERSION: u8 = 0x02;
    pub const FALLBACK_LOCKTIME: u8 = 0x03;
    pub const INPUT_COUNT: u8 = 0x04;
    pub const OUTPUT_COUNT: u8 = 0x05;
    pub const TX_MODIFIABLE: u8 = 0x06;
    pub const VERSION: u8 = 0xFB;
}

pub mod input {
    pub const NON_WITNESS_UTXO: u8 = 0x00;
    pub const WITNESS_UTXO: u8 = 0x01;
    pub const SIGHASH_TYPE: u8 = 0x03;
    pub const REDEEM_SCRIPT: u8 = 0x04;
    pub const WITNESS_SCRIPT: u8 = 0x05;
    pub const FINAL_SCRIPTSIG: u8 = 0x07;
    pub const FINAL_SCRIPTWITNESS: u8 = 0x08;
    pub const PREVIOUS_TXID: u8 = 0x0e;
    pub const OUTPUT_INDEX: u8 = 0x0f;
    pub const SEQUENCE: u8 = 0x10;
    pub const REQUIRED_TIME_LOCKTIME: u8 = 0x11;
    pub const REQUIRED_HEIGHT_LOCKTIME: u8 = 0x12;
    pub const TAP_KEY_SIG: u8 = 0x13;
    pub const TAP_SCRIPT_SIG: u8 = 0x14;
    pub const TAP_LEAF_SCRIPT: u8 = 0x15;
}

pub mod output {
    pub const AMOUNT: u8 = 0x03;
    pub const SCRIPT: u8 = 0x04;
}
