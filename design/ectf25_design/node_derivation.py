from .key_gen import KeyNode
from .util import compute_chacha_block, verify_timestamp


# TODO (sebastian): verify that these changes work
def generate_node(node: KeyNode, time: int):
    for depth in range(64):
        # iterate over the bits of `time` from highest-order to lowest-order
        direction = (time >> (63 - depth)) & 1

        # 0 = left, 1 = right
        node = node.gen_left_node() if direction == 0 else node.gen_right_node()
    return node


def derive_node(root_key: bytes, time: int) -> KeyNode:
    """
    Generates the specific key node associated with a certain time
    """

    verify_timestamp(time)

    root_node = KeyNode(root_key, 0, 2**64 - 1)

    return generate_node(root_node, time)
