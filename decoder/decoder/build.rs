use argon2::{Algorithm, Argon2, Params, Version};
use ed25519_dalek::{SecretKey, SigningKey, PUBLIC_KEY_LENGTH};
use rand::rngs::ThreadRng;
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

fn private_key_to_public_key(private_key: &SecretKey) -> [u8; PUBLIC_KEY_LENGTH] {
    SigningKey::from_bytes(private_key)
        .verifying_key()
        .to_bytes()
}

#[derive(Debug, Deserialize)]
struct ChannelSecrets {
    root_key: [u8; 32],
    private_key: [u8; 32],
}

#[derive(Debug, Deserialize)]
struct GlobalSecrets {
    subscribe_root_key: [u8; 32],
    subscribe_private_key: [u8; 32],
    channels: HashMap<usize, ChannelSecrets>,
}

// this build script just parses ap ectf params from inc/ectf_params.h into a rust file $OUT_DIR/ectf_params.rs
fn main() {
    force_rerun();
    let secrets_file =
        &std::env::var("LOCAL_SECRETS_FILE").unwrap_or("/secrets/secrets.json".to_string());
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

    rust_code.push_str(&format!("pub const DECODER_ID: u32 = {};\n", decoder_id));

    let mut add_bytes = |name, data: &[u8]| {
        rust_code.push_str(&format!(
            "pub const {}: [u8; {}] = {:?};\n",
            name,
            data.len(),
            data
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

    // this start address is pass the end of the address max size binary can load to from bootloader
    // (0x10046000) there is an extra page in between just in case
    // leave 8 pages after end, since we can have 8 pages of data
    let flash_data_range_start = 0x10048000;
    let flash_data_range_end = 0x1007c000 - 8 * 0x2000;
    // the address where we store state that can change in flash at
    // must be multiple of 128
    let flash_data_addr = rand::thread_rng()
        .gen_range((flash_data_range_start / 0x2000)..(flash_data_range_end / 0x2000))
        * 0x2000;

    rust_code.push_str(&format!(
        "pub const FLASH_DATA_ADDR: usize = {flash_data_addr};\n"
    ));

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_path.join("ectf_params.rs"), rust_code).unwrap();

    // do compile time aslr
    let mut rng = rand::thread_rng();

    let flash_length = 0x00038000;
    let ram_length = 0x00020000;
    let flash_origin = 0x1000e000;
    let ram_origin = 0x20000000;

    let stack_start = ram_origin + (ram_length / 4) + gen_addr(0, ram_length / 2, &mut rng);

    let sentry = 0x1000e200;

    // by default ap is 192 - 193 kib
    // this leaves about 12 kib of extra space in maximum size
    let textoffset = gen_addr(0, 0x4000, &mut rng);
    let rodataoffset = 0;
    let dataoffset = gen_addr(0, 0x1000, &mut rng);
    let bssoffset = gen_addr(0, ram_length / 8, &mut rng);

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
