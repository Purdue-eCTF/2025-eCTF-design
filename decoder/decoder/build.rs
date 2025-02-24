use argon2::{Algorithm, Argon2, Params, Version};
use ed25519_dalek::{SecretKey, SigningKey, PUBLIC_KEY_LENGTH};
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn force_rerun() {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(".cargo_build_rs_rerun")
        .expect("could not create rerun file");

    write!(file, "a").unwrap();

    println!("cargo:rerun-if-changed=.cargo_build_rs_rerun");
}

/// Generate an address that is a multiple of 8.
///
/// Most need to be a multiple of 4, but stack needs to be a multiple of 8).
fn gen_addr(start: u32, end: u32, rng: &mut ThreadRng) -> u32 {
    rng.gen_range((start / 8 + 1)..(end / 8)) * 8
}

/// Parse a string into 4 byte unsigned decoder id
fn parse_decoder_id(n: &str) -> u32 {
    let n_parsed = if let Some(n) = n.strip_prefix("0x") {
        u32::from_str_radix(n, 16)
    } else {
        n.parse::<u32>()
    };
    n_parsed.expect("could not parse component id")
}

/// Return bytes of the Ed25519 public key for the given private key.
fn private_key_to_public_key(private_key: &SecretKey) -> [u8; PUBLIC_KEY_LENGTH] {
    SigningKey::from_bytes(private_key)
        .verifying_key()
        .to_bytes()
}

const EMERGENCY_CHANNEL_ID: u8 = 8;

/// Secrets keys in global secrets file.
///
/// See python secret generation for details.
#[derive(Debug, Deserialize)]
struct GlobalSecrets {
    subscribe_root_key: [u8; 32],
    subscribe_private_key: [u8; 32],
    channels: HashMap<usize, ChannelSecrets>,
}

/// Secrets for an individual channel.
///
/// See python secret generation for details.
#[derive(Debug, Deserialize)]
struct ChannelSecrets {
    internal_id: u8,
    root_key: [u8; 32],
    private_key: [u8; 32],
}

// this build script just parses ap ectf params from inc/ectf_params.h into a rust file $OUT_DIR/ectf_params.rs
// also does compile time layout randomization
fn main() {
    force_rerun();

    let secrets_file =
        &std::env::var("LOCAL_SECRETS_FILE").unwrap_or("/global.secrets".to_string());
    let global_secrets = std::fs::read_to_string(secrets_file)
        .map_err(|err| format!("{err}: {secrets_file}"))
        .unwrap();

    let secrets: GlobalSecrets =
        serde_json::from_str(&global_secrets).expect("could not deserialize global secrets");

    let decoder_id =
        parse_decoder_id(&std::env::var("DECODER_ID").expect("Decoder ID not specified"));

    // first generate subscription key, which is using argon2
    // these params are the one used by the python library we picked
    // they are highed paramaters then the default ones of the rust `argon2` library
    let params = Params::new(65536, 3, 4, None).unwrap();
    let hasher = Argon2::new(Algorithm::Argon2id, Version::default(), params);

    let mut subscription_key = [0; Params::DEFAULT_OUTPUT_LEN];
    hasher
        .hash_password_into(
            &decoder_id.to_le_bytes(),
            &secrets.subscribe_root_key,
            &mut subscription_key,
        )
        .expect("failed to create device subscription key");

    // generate rust code with necessary constants
    let mut rust_code = String::new();

    rust_code.push_str(&format!("pub const DECODER_ID: u32 = {decoder_id};\n"));

    let mut add_bytes = |name, data: &[u8]| {
        rust_code.push_str(&format!(
            "pub const {name}: [u8; {}] = {data:?};\n",
            data.len()
        ));
    };

    add_bytes("SUBSCRIPTION_ENC_KEY", &subscription_key);
    add_bytes(
        "SUBSCRIPTION_PUBLIC_KEY",
        &private_key_to_public_key(&secrets.subscribe_private_key),
    );
    add_bytes("CHANNEL0_ENC_KEY", &secrets.channels[&0].root_key);
    add_bytes(
        "CHANNEL0_PUBLIC_KEY",
        &private_key_to_public_key(&secrets.channels[&0].private_key),
    );

    // preload channel public keys so subscriptions are smaller
    let mut channel_keys = secrets.channels.iter()
        .filter(|(_, channel_info)| channel_info.internal_id != EMERGENCY_CHANNEL_ID)
        .collect::<Vec<_>>();

    channel_keys.sort_by_key(|(_, channel_key)| channel_key.internal_id);
    let channel_public_keys = channel_keys.iter()
        .map(|(_, channel_key)| private_key_to_public_key(&channel_key.private_key))
        .collect::<Vec<_>>();

    let channel_external_ids = channel_keys.iter()
        .map(|(external_channel_id, _)| **external_channel_id)
        .collect::<Vec<_>>();

    rust_code.push_str(&format!(
        "pub const CHANNEL_PUBLIC_KEYS: [[u8; 32]; {}] = {channel_public_keys:?};\n",
        channel_public_keys.len(),
    ));

    rust_code.push_str(&format!(
        "pub const CHANNEL_EXTERNAL_IDS: [u32; {}] = {channel_external_ids:?};\n",
        channel_external_ids.len(),
    ));


    // do compile time aslr
    // ASLR randomizes as following:
    //
    // Flash:
    // Ectf bootloader only loads our program on a certain part of flash.
    // Flash starts at 0x10000000, our binary is loaded at 0x1000e000
    // Our binary can be up to 0x38000 bytes long, so it ends at 0x10046000, which is the 35th page of flash
    // (meaning 36th page and on we are free to use for other data storage, such as subscription data)
    //
    // There are 64 pages total, and there was some vulnerability with the last page last year,
    // so we use for subscription data storage pages 37 to 62 inclusive
    // (gap of one page after code, gap of one page on the other side before the vulnerable / info page).
    // We pick 8 random pages from these pages to store subscription data on.
    //
    // |--------------------------------------------------------------------------------|
    // | .vector_table: always 0x1000e000 (flash origin)                                |
    // |--------------------------------------------------------------------------------|
    // | .entry: always 0x1000e200 (flash origin + 0x200) (dictated by ectf bootloader) |
    // |--------------------------------------------------------------------------------|
    // |                                                                                |
    // | Large random gap                                                               |
    // | (Technically this gap is part of the .text section,                            |
    // | but it is uninitialized bytes)                                                 |
    // |                                                                                |
    // |--------------------------------------------------------------------------------|
    // | .text                                                                          |
    // | .rodata                                                                        |
    // | .data (initial values of data, copied to memory by .entry function)            |
    // |--------------------------------------------------------------------------------|
    //
    // RAM:
    // |--------------------------------------------------------------------------------|
    // | .stack: top of stack is at stack_start, grows down                             |
    // | (will have at least 1/4 ram size to grow down in ram)                          |
    // |                                                                                |
    // | stack_start: base of stack, randomly placed in middle half of flash            |
    // |--------------------------------------------------------------------------------|
    // |                                                                                |
    // | Random gap placed between top of stack and .data                               |
    // |                                                                                |
    // |--------------------------------------------------------------------------------|
    // | .data: placed in ram at stack_start + random offset                            |
    // |--------------------------------------------------------------------------------|
    // |                                                                                |
    // | Random gap placed between .data and .bss                                       |
    // |                                                                                |
    // |--------------------------------------------------------------------------------|
    // | .bss                                                                           |
    // |--------------------------------------------------------------------------------|
    const FLASH_START: usize = 0x10000000;
    const FLASH_PAGE_SIZE: usize = 8192;

    let mut rng = rand::thread_rng();

    let flash_length = 0x00038000;
    let ram_length = 0x00020000;
    let flash_origin = 0x1000e000;
    let ram_origin = 0x20000000;

    //  originally ram_length / 4 + gen_addr(ram_length / 2)
    // key node cache is very big though so needed more stack space
    // TODO: see if needed stack space can be reduced and this could be more randomized
    let stack_start = ram_origin + (3 * ram_length / 4) + gen_addr(0, ram_length / 8, &mut rng);

    let sentry = 0x1000e200;

    // by default decoder is ~92 KiB
    // flash available for bootloader to flash our code is 224 KiB
    // if we randomize up to 100 KiB gap in text, we have a margin of error of 32 KiB
    let textoffset = gen_addr(0, 0x19000, &mut rng);
    let rodataoffset = 0;
    // this random offset is a bit small since it impacts final binary size
    // don't want to waste too much space on that, since stack_start randomization already affects data placements
    // 0x2000 is 8KiB, still under 32 KiB margin of error
    let dataoffset = gen_addr(0, 0x2000, &mut rng);
    let bssoffset = gen_addr(0, ram_length / 16, &mut rng);

    // now determine which 8 pages to use for storing flash data
    // possible pages are 37 to 62 inclusive as described above
    let data_pages = 37..=62;
    let used_data_pages = data_pages
        .choose_multiple(&mut rng, 8)
        .iter()
        .map(|page_number| FLASH_START + page_number * FLASH_PAGE_SIZE)
        .collect::<Vec<_>>();

    rust_code.push_str(&format!(
        "pub const FLASH_DATA_ADDRS: [usize; 8] = {used_data_pages:?};\n",
    ));

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_path.join("ectf_params.rs"), rust_code).unwrap();

    let memory_x = format!("
        MEMORY {{
            /* ROM        (rx) : ORIGIN = 0x00000000, LENGTH = 0x00010000 */
            FLASH      (rx) : ORIGIN = {flash_origin:#x}, LENGTH = {flash_length:#x} /* 448KB Flash */
            RAM      (rwx) : ORIGIN = {ram_origin:#x}, LENGTH = {ram_length:#x} /* 128kB SRAM */
        }}
        
        /* Bootloader jumps to this address to start the ap */
        _sentry = {sentry:#x};

        rodataoffset = {rodataoffset:#x};

        _stack_start = {stack_start:#x};
        dataoffset = {dataoffset:#x};
        bssoffset = {bssoffset:#x};
        textoffset = {textoffset:#x};
    ");

    File::create(out_path.join("memory.x"))
        .unwrap()
        .write_all(memory_x.as_bytes())
        .unwrap();

    println!("cargo:rustc-link-search={}", out_path.display());

    println!("cargo:rustc-link-arg=--nmagic");

    // FIXME: make sure we are not accidently using cortex-m-rt linker script
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-changed=link.x");
}
