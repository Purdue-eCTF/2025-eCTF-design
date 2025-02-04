from .key_gen import KeyNode
from .util import compute_chacha_block, verify_timestamp


def generate_node(node: KeyNode, time: list[str]):
    k = 0
    while len(time) > 0:
        direction = time.pop()
        k += 1
        if direction == "0":
            node = node.gen_left_node(k)
        elif direction == "1":
            node = node.gen_right_node(k)
        else:
            return None
    return node


def format_time(timestamp: int) -> str:
    # tree algorithm takes a goofy queue representation of the timestamp
    return list(bin(timestamp)[2:].rjust(64, "0")[::-1])


def derive_node(root_key: bytes, time: int) -> KeyNode:
    """
    Generates the specific key node associated with a certain time
    """

    verify_timestamp(time)

    root_node = KeyNode(key=root_key)

    return generate_node(root_node, format_time(time))
