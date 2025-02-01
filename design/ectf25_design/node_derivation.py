from .util import verify_timestamp, compute_chacha_block
import os

class KeyNode:
    def __init__(self, key=None, left=None, right=None, time=None, depth=None):
        self.left = left
        self.right = right
        self.key = key
        self.time = time
        self.depth = depth
    
    def gen_left_node(self, depth):
        leftcha = compute_chacha_block(self.key)[:len(self.key)]
        if (self.time == None): 
            self.time = [0]
            self.left = KeyNode(key = leftcha, time=self.time, depth=depth)
            return self.left
        self.left = KeyNode(key = leftcha, time=self.time + [0], depth=depth)
        return self.left
    
    def gen_right_node(self, depth):
        rightcha = compute_chacha_block(self.key)[:len(self.key)]
        if (self.time == None): 
            self.time = [1]
            self.right = KeyNode(key = rightcha, time=self.time, depth=depth)
            return self.right
        self.right = KeyNode(key = rightcha, time=self.time + [1], depth=depth)
        return self.right
    
def generate_node(node: KeyNode, time):
    k = 0
    while len(time) > 0:
        direction = time.pop()
        k += 1
        if direction == '0':
            node = node.gen_left_node(k)
        elif direction == '1':
            node = node.gen_right_node(k)
        else:
            return None
    return node
            
    
def format_time(timestamp: int) -> str:
    # tree algorithm takes a goofy queue representation of the timestamp 
    return list(bin(timestamp)[2:].rjust(64, '0')[::-1])
    
def derive_node(root_key: bytes, time: int) -> KeyNode:
    """
    Generates the specific key node associated with a certain time
    """

    verify_timestamp(time)

    root_node = KeyNode(key = root_key)

    return generate_node(root_node, format_time(time))

