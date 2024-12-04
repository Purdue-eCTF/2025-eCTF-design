use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::env;

use rand::rngs::ThreadRng;
use deployment::{SecretDb, parse_component_id, generate_encrypted_rust_const, force_rerun, LineParser};
use rand::Rng;

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

    let mut rust_code = String::new();

    let secret_db = SecretDb::new("../secret_db.sqlite")
        .expect("could not open the secret db");

    let global_secrets = secret_db.get_global_secrets()
        .expect("could not get global secrets");

    let mut attestation_location = None;
    let mut attestation_date = None;
    let mut attestation_customer = None;

    for line in ectf_params.lines().map(str::trim) {
        let mut parser = LineParser {
            data: line,
        };

        if let Some("#define") = parser.take_token() {
            match parser.take_token() {
                Some("COMPONENT_ID") => {
                    let num = parser.take_token().expect("no parameter for COMPONENT_ID");
                    let component_id = parse_component_id(num);

                    let component_keypair = secret_db.get_component_keypair(component_id)
                        .expect("could not get component keypair");

                    rust_code.push_str(&format!("pub const COMPONENT_ID: u32 = {};\n", num));
                    rust_code.push_str(&format!("pub const BUILD_ID: u32 = {};\n", component_keypair.build_id));
                    rust_code.push_str(&format!("pub const COMPONENT_PUBKEY: [u8; 32] = {:?};\n", component_keypair.keypair.pubkey));
                    rust_code.push_str(&format!("pub const COMPONENT_PRIVKEY: [u8; 32] = {:?};\n", component_keypair.keypair.privkey));
                },
                Some("COMPONENT_BOOT_MSG") => {
                    let boot_msg = parser.take_string().expect("no parameter for COMPONENT_BOOT_MSG");

                    rust_code.push_str(&generate_encrypted_rust_const(
                        "boot_message",
                        boot_msg.as_bytes(),
                        global_secrets.boot_data_enc_key,
                    ));
                },
                Some("ATTESTATION_LOC") => {
                    attestation_location = Some(
                        parser.take_string().expect("no parameter for ATTESTATION_LOC").to_owned()
                    );
                },
                Some("ATTESTATION_DATE") => {
                    attestation_date = Some(
                        parser.take_string().expect("no parameter for ATTESTATION_LOC").to_owned()
                    );
                },
                Some("ATTESTATION_CUSTOMER") => {
                    attestation_customer = Some(
                        parser.take_string().expect("no parameter for ATTESTATION_LOC").to_owned()
                    );
                },
                _ => (),
            }
        }
    }

    rust_code.push_str(&format!("pub const HMAC_KEY: [u8; 32] = {:?};\n", global_secrets.hmac_key));
    rust_code.push_str(&format!("pub const BOOT_CR_KEY: [u8; 32] = {:?};\n", global_secrets.boot_cr_key));
    rust_code.push_str(&format!("pub const AP_PUBKEY: [u8; 32] = {:?};\n", global_secrets.ap_keypair.pubkey));

    let attestation_string = format!(
        "LOC>{}\nDATE>{}\nCUST>{}\n",
        attestation_location.unwrap(),
        attestation_date.unwrap(),
        attestation_customer.unwrap(),
    );

    rust_code.push_str(&generate_encrypted_rust_const(
        "attestation_data",
        attestation_string.as_bytes(),
        global_secrets.attestation_data_enc_key,
    ));

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
        
        // by default component is 146 - 147 kib
        // this leaves about 8 kib of space in maximum size configuration
        let textoffset = gen_addr(0, 0x10000, &mut rng);
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