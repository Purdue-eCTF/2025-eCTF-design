import base64
import json
import struct
from dataclasses import dataclass
from typing import Dict, List, Self

from argon2 import PasswordHasher
from Crypto.Cipher import ChaCha20, ChaCha20_Poly1305
from Crypto.Random import get_random_bytes
from Crypto.Signature import eddsa

def random(n: int) -> bytes:
    """Generates `n` cryptographically secure random bytes."""

    return get_random_bytes(n)


def bytes_to_eddsa_key(key: bytes) -> eddsa.EdDSASigScheme:
    """Constructs an Ed25519 signing key for a private key of bytes."""

    return eddsa.new(eddsa.import_private_key(key), "rfc8032")


def verify_timestamp(timestamp: int):
    """Asserts the given integer is a valid timestamp."""

    assert timestamp >= 0
    assert timestamp < (2**64)


def verify_decoder(decoder_id: int):
    """Asserts the given decoder id is valid."""

    assert decoder_id >= 0
    assert decoder_id < (2**32)


@dataclass
class ChannelKey:
    """Keys used for a specific channel."""

    # root of key tree used for generating encryption keys.
    # For channel 0 there is no key tree, this is just encryption key itself.
    root_key: bytes

    # Ed25519 private key used to sign tv frames.
    private_key: bytes

    def signing_key(self) -> eddsa.EdDSASigScheme:
        return bytes_to_eddsa_key(self.private_key)

    def public_key_bytes(self) -> bytes:
        pub_key = eddsa.import_private_key(self.private_key).public_key()
        raw = pub_key.export_key(format="raw")
        assert len(raw) == 32
        return raw

    @classmethod
    def generate(cls) -> Self:
        return cls(
            root_key=random(32),
            private_key=random(32),
        )


@dataclass
class GlobalSecrets:
    """All global secrets for the satelite tv system."""

    # 256 bit secret used to derive all subscription encryption keys.
    subscribe_root_key: bytes

    # Ed25519 private key used to sign subscription payloads.
    subscribe_private_key: bytes

    channels: dict[int, ChannelKey]

    def subscribe_signing_key(self) -> eddsa.EdDSASigScheme:
        return bytes_to_eddsa_key(self.subscribe_private_key)

    def subscription_key_for_decoder(self, decoder_id: int) -> bytes:
        # decoder id must be 4 byte unsigned integer
        verify_decoder(decoder_id)
        decoder_id_bytes = struct.pack("<I", decoder_id)

        # use argon2id to derive the decoder subscription key
        # these are the default values, just explicitly specified, and with different salt len
        hasher = PasswordHasher(
            time_cost=3,
            memory_cost=65536,
            parallelism=4,
            hash_len=32,
            salt_len=32,
        )
        decoder_id_hash = hasher.hash(decoder_id_bytes, salt=self.subscribe_root_key)

        # returned hash string has many paramaters encoded as well, delimeted by $
        # last element is the hash itself
        # have to add 1 = to base64 because python base64 padding is always mandatory apparently
        return base64.b64decode(decoder_id_hash.split("$")[-1] + "=")

    @classmethod
    def generate(cls, channel_ids: list[int]) -> Self:
        channels = {}
        # channel 0 always exists
        channels[0] = ChannelKey.generate()

        for channel_id in channel_ids:
            channels[channel_id] = ChannelKey.generate()

        return cls(
            subscribe_root_key=random(32),
            subscribe_private_key=random(32),
            channels=channels,
        )

    def to_json(self) -> str:
        return json.dumps({
            "subscribe_root_key": list(self.subscribe_root_key),
            "subscribe_private_key": list(self.subscribe_private_key),
            "channels": {
                channel_id: {
                    "root_key": list(channel.root_key),
                    "private_key": list(channel.private_key),
                }
                for channel_id, channel in self.channels.items()
            },
        })

    @classmethod
    def from_json(cls, raw_data: str) -> Self:
        data: dict = json.loads(raw_data)

        return cls(
            subscribe_root_key=bytes(data["subscribe_root_key"]),
            subscribe_private_key=bytes(data["subscribe_private_key"]),
            channels={
                int(channel_id): ChannelKey(
                    root_key=bytes(channel_json["root_key"]),
                    private_key=bytes(channel_json["private_key"]),
                )
                for channel_id, channel_json in data["channels"].items()
            },
        )


def encrypt_payload(
    data: bytes,
    associated_data: bytes,
    symmetric_key: bytes,
    private_key: eddsa.EdDSASigScheme,
):
    """
    This function encrypts and signs all sensitive payloads sent to the decoder,
    including subsciptions and satellite data.

    `data` will be encrypted, while `associated_data` will remain unencrypted,
    but still verified by decoder.

    Payload Format:
    |-----------------------------------------------|
    | Ed25519 Signature: 64 bytes                   |
    |-----------------------------------------------|
    | XChaCha20Poly1305 Nonce: 24 bytes             |
    |-----------------------------------------------|
    | Poly1305 Tag: 16 bytes                        |
    |-----------------------------------------------|
    | Ciphertext: `len(data)` bytes                 |
    |-----------------------------------------------|
    | Associated Data: `len(associated_data)` bytes |
    |-----------------------------------------------|
    """

    nonce = random(24)
    cipher = ChaCha20_Poly1305.new(key=symmetric_key, nonce=nonce)
    # associated data must be fed in before encrypting
    cipher.update(associated_data)
    (ciphertext, poly1305_tag) = cipher.encrypt_and_digest(data)

    # now sign the payload
    # must sign nonce and tag too to prevent attacker with leaked
    # key from changing nonce to get different decryption
    payload_to_sign = nonce + poly1305_tag + ciphertext + associated_data

    # we are using eddsa in 'pure' mode (no hashing before signing, only using the 2 hashes in eddsa itself)
    signature = private_key.sign(payload_to_sign)

    return signature + payload_to_sign


def compute_chacha_block(data: bytes) -> bytes:
    """Computes 1 length doubling chacha block."""

    assert len(data) == 32

    cipher = ChaCha20.new(key=data, nonce=b"\0" * 8)

    # just encrypt 0s to get the keystream
    return cipher.encrypt(b"\0" * 64)
