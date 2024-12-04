use std::{fs::OpenOptions, io::Write, path::Path};

use ed25519_dalek::{SigningKey, SECRET_KEY_LENGTH};
use chacha20poly1305::{aead::Aead, AeadCore, ChaCha20Poly1305, KeyInit};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection, Error, ErrorCode};
use thiserror::Error;

const CHACHA20_TAG_SIZE: usize = 16;
const NUM_COMPONENT_KEYPAIRS: usize = 32;

#[derive(Debug, Error)]
pub enum DeploymentError {
    #[error("Sqlite error: {0}")]
    SqliteError(#[from] Error),
    #[error("The global secrets were not yet created")]
    NoGlobalSecrets,
    #[error("Could not generate secrets for the given component")]
    NoComponentSecrets,
    #[error("Too many components were built")]
    TooManyComponent,
}

#[derive(Debug)]
pub struct KeyPair {
    pub pubkey: [u8; 32],
    pub privkey: [u8; 32],
}

impl KeyPair {
    fn generate() -> Self {
        let key = SigningKey::generate(&mut OsRng);
        let keypair = key.to_keypair_bytes();

        KeyPair {
            pubkey: keypair[SECRET_KEY_LENGTH..].try_into().unwrap(),
            privkey: key.to_bytes(),
        }
    }
}

#[derive(Debug)]
pub struct ComponentKeyPair {
    pub build_id: u32,
    pub keypair: KeyPair,
}

#[derive(Debug)]
pub struct GlobalSecrets {
    pub hmac_key: [u8; 32],
    pub attestation_data_enc_key: [u8; 32],
    /// Encryption key for boot challenge response process
    pub boot_cr_key: [u8; 32],
    pub boot_data_enc_key: [u8; 32],
    pub ap_keypair: KeyPair,
}

impl GlobalSecrets {
    fn generate() -> Self {
        GlobalSecrets {
            hmac_key: generate_random_bytes(),
            attestation_data_enc_key: generate_random_bytes(),
            boot_cr_key: generate_random_bytes(),
            boot_data_enc_key: generate_random_bytes(),
            ap_keypair: KeyPair::generate(),
        }
    }
}

pub struct SecretDb {
    db: Connection,
}

impl SecretDb {
    /// Gets an instance of the global secrets, the path to the global secret database
    pub fn new<P: AsRef<Path>>(secret_db: P) -> Result<Self, DeploymentError> {
        let db = Connection::open(secret_db)?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS global_secrets (
                hmac_key BLOB NOT NULL,
                attestation_data_enc_key BLOB NOT NULL,
                boot_cr_key BLOB NOT NULL,
                boot_data_enc_key BLOB NOT NULL,
                ap_pubkey BLOB NOT NULL,
                ap_privkey BLOB NOT NULL
            )",
            (),
        )?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS component_keypairs (
                build_id INTEGER NOT NULL PRIMARY KEY,
                in_use BOOLEAN NOT NULL,
                pubkey BLOB NOT NULL,
                privkey BLOB NOT NULL,
                UNIQUE(build_id)
            )",
            (),
        )?;

        db.execute(
            "CREATE TABLE IF NOT EXISTS component_build_ids (
                component_id INTEGER NOT NULL PRIMARY KEY,
                build_id INTEGER NOT NULL references component_keypairs(build_id),
                UNIQUE(component_id)
            )",
            (),
        )?;

        Ok(SecretDb {
            db,
        })
    }

    /// This method is ran by the deployment makefile
    pub fn generate_global_secret(&self) -> Result<(), DeploymentError> {
        let secrets = GlobalSecrets::generate();

        self.db.execute(
            "INSERT INTO global_secrets (hmac_key, attestation_data_enc_key, boot_cr_key, boot_data_enc_key, ap_pubkey, ap_privkey)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                secrets.hmac_key.as_slice(),
                secrets.attestation_data_enc_key.as_slice(),
                secrets.boot_cr_key.as_slice(),
                secrets.boot_data_enc_key.as_slice(),
                secrets.ap_keypair.pubkey.as_slice(),
                secrets.ap_keypair.privkey.as_slice(),
            ),
        )?;

        for _ in 0..NUM_COMPONENT_KEYPAIRS {
            let key_pair = KeyPair::generate();

            loop {
                let build_id = OsRng.next_u32();
                let result = self.db.execute(
                    "INSERT INTO component_keypairs (build_id, in_use, pubkey, privkey)
                    VALUES (?1, FALSE, ?2, ?3)",
                    (
                        build_id,
                        key_pair.pubkey,
                        key_pair.privkey,
                    ),
                );

                match result {
                    Err(rusqlite::Error::SqliteFailure(error, _))
                        if error.code == ErrorCode::ConstraintViolation => {
                        // this means the build id was generated was not unique, so try again
                        continue;
                    },
                    Ok(_) => break,
                    Err(error) => return Err(DeploymentError::SqliteError(error)),
                }
            }
        }

        Ok(())
    }

    pub fn get_global_secrets(&self) -> Result<GlobalSecrets, DeploymentError> {
        let mut statement = self.db.prepare(
            "SELECT hmac_key, attestation_data_enc_key, boot_cr_key, boot_data_enc_key, ap_pubkey, ap_privkey FROM global_secrets",
        )?;

        let row = statement.query_map([], |row| {
            Ok(GlobalSecrets {
                hmac_key: row.get(0)?,
                attestation_data_enc_key: row.get(1)?,
                boot_cr_key: row.get(2)?,
                boot_data_enc_key: row.get(3)?,
                ap_keypair: KeyPair {
                    pubkey: row.get(4)?,
                    privkey: row.get(5)?,
                }
            })
        })?.next().ok_or(DeploymentError::NoGlobalSecrets)??;

        Ok(row)
    }

    /// Gets a list of all the component keypairs
    pub fn get_all_component_keypairs(&self) -> Result<Vec<ComponentKeyPair>, DeploymentError> {
        let mut statement = self.db.prepare(
            "SELECT build_id, pubkey, privkey FROM component_keypairs",
        )?;

        let result: Result<Vec<_>, rusqlite::Error> = statement.query_map([], |row| {
            Ok(ComponentKeyPair {
                build_id: row.get(0)?,
                keypair: KeyPair {
                    pubkey: row.get(1)?,
                    privkey: row.get(2)?,
                },
            })
        })?.collect();

        Ok(result?)
    }

    pub fn get_component_keypair(&self, component_id: u32) -> Result<ComponentKeyPair, DeploymentError> {
        let mut statement = self.db.prepare(
            "SELECT component_build_ids.build_id, component_keypairs.pubkey, component_keypairs.privkey
            FROM component_build_ids
            INNER JOIN component_keypairs ON
            component_build_ids.build_id = component_keypairs.build_id
            WHERE component_build_ids.component_id = ?1",
        )?;

        let componeny_keypair = statement.query_map([component_id], |row| {
            Ok(ComponentKeyPair {
                build_id: row.get(0)?,
                keypair: KeyPair {
                    pubkey: row.get(1)?,
                    privkey: row.get(2)?,
                },
            })
        })?.next();

        // this components keypair is already defined
        if let Some(keypair) = componeny_keypair {
            return Ok(keypair?);
        }

        // this components keypair is not defined, make it defined
        let mut statement = self.db.prepare(
            "UPDATE component_keypairs
            SET in_use = TRUE
            FROM (SELECT build_id, in_use FROM component_keypairs WHERE in_use = FALSE LIMIT 1) as update_row
            WHERE update_row.build_id = component_keypairs.build_id
            RETURNING build_id, pubkey, privkey"
        )?;

        let component_keypair = statement.query_map([], |row| {
            Ok(ComponentKeyPair {
                build_id: row.get(0)?,
                keypair: KeyPair {
                    pubkey: row.get(1)?,
                    privkey: row.get(2)?,
                },
            })
        })?.next().ok_or(DeploymentError::TooManyComponent)??;

        self.db.execute(
            "INSERT INTO component_build_ids (component_id, build_id)
            VALUES (?1, ?2)",
            (component_id, component_keypair.build_id),
        )?;

        Ok(component_keypair)
    }
}

pub fn generate_random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0; N];
    OsRng.fill_bytes(&mut out);
    out
}

pub fn parse_component_id(n: &str) -> u32 {
    if n.starts_with("0x") {
        u32::from_str_radix(&n[2..], 16)
            .expect("could not parse component id")
    } else {
        n.parse::<u32>()
            .expect("could not parse component id")
    }
}

/// Generates a string with a rust function that returns encrypted data for the given message and key
pub fn generate_encrypted_rust_const(name: &str, data: &[u8], key: [u8; 32]) -> String {
    let EncryptResult {
        ciphertext,
        nonce,
        tag,
    } = encrypt(data, key)
        .expect(&format!("could not encrypt {name}"));

    format!("#[inline]
    pub fn encrypted_{name}() -> EncryptedData<{}> {{
        EncryptedData {{
            ciphertext: {ciphertext:?}.into(),
            tag: {tag:?},
            nonce: {nonce:?},
        }}
    }}", ciphertext.len())
}

pub struct EncryptResult {
    ciphertext: Vec<u8>,
    nonce: [u8; 12],
    tag: [u8; CHACHA20_TAG_SIZE],
}

pub fn encrypt(message: &[u8], key: [u8; 32]) -> Result<EncryptResult, chacha20poly1305::Error> {
    let cipher = ChaCha20Poly1305::new(&key.into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher.encrypt(&nonce, message)?;

    let tag = &ciphertext[(ciphertext.len() - CHACHA20_TAG_SIZE)..];
    let data = &ciphertext[..(ciphertext.len() - CHACHA20_TAG_SIZE)];

    Ok(EncryptResult {
        ciphertext: Vec::from(data),
        nonce: nonce.into(),
        tag: tag.try_into().unwrap(),
    })
}

pub fn force_rerun() {
    let mut file = OpenOptions::new().create(true).write(true).open(".cargo_build_rs_rerun")
        .expect("could not create rerun file");

    write!(file, "a").unwrap();

    println!("cargo:rerun-if-changed=.cargo_build_rs_rerun");
}

pub struct LineParser<'a> {
    pub data: &'a str,
}

impl<'a> LineParser<'a> {
    pub fn take_until_index(&mut self, index: usize) -> &'a str {
        let out = &self.data[..index];
        self.data = &self.data[index..];
        out
    }

    pub fn take_token(&mut self) -> Option<&'a str> {
        if self.data.len() == 0 {
            return None;
        }

        let out = self.take_until_index(self.data.find(' ').unwrap_or(self.data.len()));
        self.data = self.data.trim_start();
        Some(out)
    }

    pub fn take_string(&mut self) -> Option<&'a str> {
        if self.data.get(..1)? != "\"" {
            return None;
        }

        self.data = &self.data[1..];

        let out = self.take_until_index(self.data.find('"')?);
        self.data = &self.data[1..];

        Some(out)
    }
}