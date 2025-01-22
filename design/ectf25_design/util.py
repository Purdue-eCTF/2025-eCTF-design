from Crypto.Random import get_random_bytes
from Crypto.Signature import eddsa
from dataclasses import dataclass
from typing import Self, Dict, List
import json

def random(n: int) -> bytes:
    return get_random_bytes(n)

def bytes_to_eddsa_key(key: bytes) -> eddsa.EdDSASigScheme:
    return eddsa.new(eddsa.import_private_key(key), "rfc8032")

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

    @classmethod
    def generate(cls) -> Self:
        return cls(
            root_key = random(32),
            private_key = random(32),
        )

@dataclass
class GlobalSecrets:
    """All global secrets for the satelite tv system."""

    # 256 bit secret used to derive all subscription encryption keys.
    subscribe_root_key: bytes

    # Ed25519 private key used to sign subscription payloads.
    subscribe_private_key: bytes

    channels: Dict[int, ChannelKey]

    def subscribe_signing_key(self) -> eddsa.EdDSASigScheme:
        return bytes_to_eddsa_key(self.subscribe_private_key)

    @classmethod
    def generate(cls, channel_ids: List[int]) -> Self:
        channels = {}
        # channel 0 always exists
        channels[0] = ChannelKey.generate()

        for id in channel_ids:
            channels[id] = ChannelKey.generate()
        
        return cls(
            subscribe_root_key = random(32),
            subscribe_private_key = random(32),
            channels = channels,
        )
    
    def to_json(self) -> str:
        return json.dumps({
            "subscribe_root_key": self.subscribe_root_key.hex(),
            "subscribe_private_key": self.subscribe_private_key.hex(),
            "channels": {id: {
                "root_key": channel.root_key.hex(),
                "private_key": channel.private_key.hex(),
            } for id, channel in self.channels.items()},
        })
    
    @classmethod
    def from_json(cls, data: str) -> Self:
        data = json.loads(data)

        return cls(
            subscribe_root_key = data["subscribe_root_key"],
            subscribe_private_key = data["subscribe_private_key"],
            channels = {id: Channel(
                root_key = channel_json["root_key"],
                private_Key = channel_json["private_key"],
            ) for id, channel_json in data["channels"]}
        )