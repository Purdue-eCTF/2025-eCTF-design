use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::convert::Infallible;
use std::env;
use rand::rngs::ThreadRng;
use rand::Rng;

use bcrypt::bcrypt;
use deployment::{force_rerun, generate_random_bytes, parse_component_id, SecretDb, LineParser};

pub fn force_rerun() {
    let mut file = OpenOptions::new().create(true).write(true).open(".cargo_build_rs_rerun")
        .expect("could not create rerun file");

    write!(file, "a").unwrap();

    println!("cargo:rerun-if-changed=.cargo_build_rs_rerun");
}

/*
 * Generate an address that is a multiple of 8
 * (Most need to be a multiple of 4, but stack needs to be a multiple of 8) 
 */
fn gen_addr(start: u32, end: u32, rng: &mut ThreadRng) -> u32 {
    rng.gen_range((start/8 + 1)..(end/8)) * 8
}

// this build script just parses ap ectf params from inc/ectf_params.h into a rust file $OUT_DIR/ectf_params.rs
fn main() {
    force_rerun();

    println!("cargo:rerun-if-changed=./inc/ectf_params.h");

    let ectf_params = std::fs::read_to_string("inc/ectf_params.h")
        .expect("no ectf params found");


    let secret_db = SecretDb::new("../secret_db.sqlite")
        .expect("could not open the secret db");

    let component_keypairs = secret_db.get_all_component_keypairs()
        .expect("could not get component keypairs");

    let mut rust_code = String::new();

    for line in ectf_params.lines().map(str::trim) {
        let mut parser = LineParser {
            data: line,
        };

        if let Some("#define") = parser.take_token() {
            match parser.take_token() {
                Some("AP_PIN") => {
                    let pin = parser.take_string().expect("no parameter for AP_ID");

                    let HashResult {
                        salt,
                        hash,
                    } = hash(pin, 8).expect("could not hash pin");

                    rust_code.push_str(&format!("pub const PIN_HASH: [u8; {}] = {:?};\n", hash.len(), hash.as_slice()));
                    rust_code.push_str(&format!("pub const PIN_SALT: [u8; {}] = {:?};\n", salt.len(), salt.as_slice()));
                },
                Some("AP_TOKEN") => {
                    let token = parser.take_string().expect("no parameter for AP_TOKEN");

                    let HashResult {
                        salt,
                        hash,
                    } = hash(token, 8).expect("could not hash pin");

                    rust_code.push_str(&format!("pub const TOKEN_HASH: [u8; {}] = {:?};\n", hash.len(), hash.as_slice()));
                    rust_code.push_str(&format!("pub const TOKEN_SALT: [u8; {}] = {:?};\n", salt.len(), salt.as_slice()));
                },
                Some("COMPONENT_IDS") => {
                    let mut ids = Vec::new();
                    while let Some(id) = parser.take_token() {
                        ids.push(id.trim_matches(','));
                    }

                    rust_code.push_str(&format!("pub const COMPONENTS: [ProvisionedComponent; {}] = [", ids.len()));

                    for id in ids {
                        let build_id = secret_db.get_component_keypair(parse_component_id(id))
                            .expect("could not get component keypair")
                            .build_id;

                        let (key_index, _) = component_keypairs.iter()
                            .enumerate()
                            .filter(|(_, keypair)| keypair.build_id == build_id)
                            .next()
                            .expect("could not find key index for the given build_id");

                        rust_code.push_str(&format!("ProvisionedComponent {{
                            component_id: {id},
                            key_index: {key_index:?},
                        }}, "));
                    }

                    rust_code.push_str("];\n");
                },
                Some("AP_BOOT_MSG") => {
                    let data = parser.take_string().expect("no parameter for AP_BOOT_MSG");

                    rust_code.push_str(&format!("pub const AP_BOOT_MSG: &str = \"{}\";\n", data));
                },
                _ => (),
            }
        }
    }

    let global_secrets = secret_db.get_global_secrets()
        .expect("could not get global secrets");

    rust_code.push_str(&format!("pub const HMAC_KEY: [u8; 32] = {:?};\n", global_secrets.hmac_key));
    rust_code.push_str(&format!("pub const ADATA_ENC_KEY: [u8; 32] = {:?};\n", global_secrets.attestation_data_enc_key));
    rust_code.push_str(&format!("pub const BOOT_CR_KEY: [u8; 32] = {:?};\n", global_secrets.boot_cr_key));
    rust_code.push_str(&format!("pub const BOOT_DATA_ENC_KEY: [u8; 32] = {:?};\n", global_secrets.boot_data_enc_key));
    rust_code.push_str(&format!("pub const AP_PUBKEY: [u8; 32] = {:?};\n", global_secrets.ap_keypair.pubkey));
    rust_code.push_str(&format!("pub const AP_PRIVKEY: [u8; 32] = {:?};\n", global_secrets.ap_keypair.privkey));

    rust_code.push_str(&format!("pub const COMPONENT_KEYS: [ComponentKey; {}] = [", component_keypairs.len()));
    for key in component_keypairs {
        rust_code.push_str(&format!("ComponentKey {{
            build_id: {:?},
            pubkey: {:?},
        }}, ", key.build_id, key.keypair.pubkey.as_slice()));
    }
    rust_code.push_str("];\n");

    // this start address is pass the end of the address max size binary can load to from bootloader
    // (0x10046000) there is an extra page in between just in case
    let flash_data_range_start = 0x10048000;
    let flash_data_range_end = 0x1007c000;
    // the address where we store state that can change in flash at
    // must be multiple of 128
    let flash_data_addr = rand::thread_rng()
        .gen_range((flash_data_range_start / 128)..(flash_data_range_end / 128)) * 128;

    rust_code.push_str(&format!("pub const FLASH_DATA_ADDR: usize = {flash_data_addr};\n"));

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::write(out_path.join("ectf_params.rs"), rust_code).unwrap();

    if env::var("TARGET").unwrap() == "thumbv7em-none-eabihf" {
        let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

        let mut rng = rand::thread_rng();

        let flash_length = 0x00038000;
        let ram_length = 0x00020000;
        let flash_origin = 0x1000e000;
        let ram_origin = 0x20000000;

        
        let stack_start = ram_origin + (ram_length/4) + gen_addr(0, ram_length / 2, &mut rng);
        
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
        
        
        File::create(out.join("memory.x"))
            .unwrap()
            .write_all(memory_x.as_bytes())
            .unwrap();

        println!("cargo:rustc-link-search={}", out.display());

        println!("cargo:rustc-link-arg=--nmagic");

        // FIXME: make sure we are not accidently using cortex-m-rt linker script
        println!("cargo:rustc-link-arg=-Tlink.x");
        println!("cargo:rerun-if-changed=link.x");
    }

    let mut post_boot_build = cc::Build::new();
    post_boot_build
        .target("thumbv7em-none-eabihf")
        .compiler("arm-none-eabi-gcc")
        .include("./post_boot")
        .flag("-w")
        .define("gcc", None)
        .file("./post_boot/post_boot.c");

    if let Some("1") = env::var("POST_BOOT_ENABLED").ok().as_deref() {
        let post_boot_code = env::var("POST_BOOT_CODE")
            .expect("POST_BOOT_CODE not defined even when post boot code was enabled");

        post_boot_build.define("POST_BOOT", Some(post_boot_code.trim_matches('\'')));
    }

    post_boot_build.compile("post_boot");
}

struct HashResult {
    salt: [u8; 16],
    hash: [u8; 24],
}

fn hash(data: &str, cost: u32) -> Result<HashResult, Infallible> {
    let salt = generate_random_bytes();
    let hash = bcrypt(cost, salt, data.as_bytes());

    Ok(HashResult {
        salt,
        hash,
    })
}
