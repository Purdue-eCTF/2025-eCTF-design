from .util import verify_timestamp, compute_chacha_block
from typing import List

#TODO check chacha because idk what I'm doing --will :)
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
    
#TODO: I think this algorithm works to generate the minimum number of nodes.
def generate_tree(node: KeyNode, min_time, max_time, k, nodes_arr):
    """
    USAGE:
    call:
        array = generate_tree(head, min_time, max_time, k, nodes_arr)
        head = a head node generated with our global secret
        min_time the binary representation of the minimum time in a stack
        max_time the binary representation of the maximum time in a stack
        k = 0
        nodes_arr = []
    """

    if min_time == None and max_time == None:
        print("over")
        return

    if (min_time and min_time[-1] == '0') and (max_time and max_time[-1] == '1'):
        if k == 63:
            nodes_arr.append(node)
            return nodes_arr
        print(f"breaking at {k} with {node.time} and {max_time[:-1]}\n" + 
              f"                              {min_time[:-1]}")
        # jack: I think this will generate 2 nodes in some cases when 1 node is fine,
        # but that is fine and other than that lgtm
        nodes_arr += gen_minNode(node.gen_left_node(k), min_time[:-1], k + 1, [])
        nodes_arr += gen_maxNode(node.gen_right_node(k), max_time[:-1], k + 1, [])
        return nodes_arr
    
    if min_time and min_time[-1] == '0':
        node.gen_left_node(k)
        return generate_tree(node.left, min_time[:-1], max_time[:-1], k+1, nodes_arr)
    elif max_time and max_time[-1] == '1':
        node.gen_right_node(k)
        return generate_tree(node.right, min_time[:-1], max_time[:-1], k+1, nodes_arr)
        
def gen_minNode(node: KeyNode, min_time, k, nodes_arr):
    k += 1
    if not ('1' in min_time): 
        nodes_arr.append(node)
        return nodes_arr
    if len(min_time) == 1:
        if min_time.pop() == '0':
            nodes_arr.append(node)
        else:
            nodes_arr.append(node.gen_right_node(k))
        return nodes_arr
    if min_time.pop() == '0':
        temp = node.gen_right_node(k)
        nodes_arr.append(temp)
        return gen_minNode(node.gen_left_node(k), min_time, k, nodes_arr)
    else:
        return gen_minNode(node.gen_right_node(k), min_time, k, nodes_arr)
    
def gen_maxNode(node: KeyNode, max_time, k, nodes_arr):
    k += 1
    if not ('0' in max_time): 
        nodes_arr.append(node)
        return nodes_arr
    if len(max_time) == 1:
        if max_time.pop() == '1':
            nodes_arr.append(node)
        else:
            nodes_arr.append(node.gen_left_node(k))
        return nodes_arr
    if max_time.pop() == '1':
        temp = node.gen_left_node(k)
        nodes_arr.append(temp)
        return gen_maxNode(node.gen_right_node(k), max_time, k, nodes_arr)
    else: 
        return gen_maxNode(node.gen_left_node(k), max_time, k, nodes_arr)

def format_time(timestamp: int) -> str:
    # code required timestamps as list of 0 and 1s, but backwards
    return list(bin(timestamp)[2:].rjust(64, '0')[::-1])

def generate_subscription_nodes(root_key: bytes, min_time: int, max_time: int) -> List[KeyNode]:
    """
    Generates a list of keynodes with subtrees spanning the time region.

    The number generated is logarithmic in size of region.
    """

    verify_timestamp(min_time)
    verify_timestamp(max_time)

    root_node = KeyNode(key = root_key)

    return generate_tree(root_node, format_time(min_time), format_time(max_time), 0, [])