from typing import List, Self

from .util import compute_chacha_block, verify_timestamp


class KeyNode:
    key: bytes
    left: Self | None
    right: Self | None
    lowest_timestamp: int
    highest_timestamp: int

    def __init__(self, key, lowest_timestamp, highest_timestamp, left=None, right=None):
        self.left = left
        self.right = right
        self.key = key
        self.lowest_timestamp = lowest_timestamp
        self.highest_timestamp = highest_timestamp

    def gen_left_node(self):
        leftcha = compute_chacha_block(self.key)[: len(self.key)]
        new_highest = (self.lowest_timestamp + self.highest_timestamp) // 2
        self.left = KeyNode(leftcha, self.lowest_timestamp, new_highest)
        return self.left

    def gen_right_node(self):
        rightcha = compute_chacha_block(self.key)[len(self.key) :]
        new_lowest = (self.lowest_timestamp + self.highest_timestamp + 1) // 2
        self.right = KeyNode(rightcha, new_lowest, self.highest_timestamp)
        return self.right


def generate_tree(
    node: KeyNode,
    min_time: int,
    max_time: int,
    left_start: int = 0,  # the lowest timestamp that this node can provide
    right_end: int = 2**64 - 1,  # the highest timestamp that this node can provide
):
    """
    Recursively calculate the smallest set of nodes to represent a time range.
    Stores the possible time range for the current node instead of using the timestamp.
    """

    # if there is only 1 node in the range, early return
    if left_start == right_end:
        assert min_time <= left_start <= max_time
        return [node]

    # split the range at the middle
    right_start = (left_start + right_end + 1) // 2
    left_end = right_start - 1

    left_side = (left_start, left_end, node.gen_left_node())
    right_side = (right_start, right_end, node.gen_right_node())

    nodes_arr = []
    for start, end, new_node in (left_side, right_side):
        if (start <= min_time <= end) or (start <= max_time <= end):
            # if this node's range is fully inside the target range, just add this node.
            if min_time <= start and end <= max_time:
                nodes_arr.append(new_node)
            # otherwise, recursively look for the correct nodes
            else:
                nodes_arr.extend(
                    generate_tree(new_node, min_time, max_time, start, end)
                )
    return nodes_arr


def generate_subscription_nodes(
    root_key: bytes,
    min_time: int,
    max_time: int,
) -> list[KeyNode]:
    """
    Generates a list of keynodes with subtrees spanning the time region.

    The number generated is logarithmic in size of region.
    """

    verify_timestamp(min_time)
    verify_timestamp(max_time)
    assert min_time <= max_time

    root_node = KeyNode(root_key, 0, 2**64 - 1)

    return generate_tree(root_node, min_time, max_time)
