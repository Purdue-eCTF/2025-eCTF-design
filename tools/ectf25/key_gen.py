from Crypto.Cipher import ChaCha20_Poly1305
from Crypto.Random import get_random_bytes
import code
from secrets import SOMETHING_HERE #yeah idk what this should be

nodes_arr = []

#TODO check chacha because idk what I'm doing --will :)
class KeyNode:
    def __init__(self, chacha=None, left=None, right=None, time=None, depth=None):
        self.left = left
        self.right = right
        self.chacha = chacha
        self.time = time
        self.depth = depth
    
    def gen_left_node(self, depth):
        leftcha = ChaCha20_Poly1305.new(self.chacha[:len(self.chacha)]//2)
        if (self.time == None): 
            self.time = [0]
            self.left = KeyNode(chacha = leftcha, time=self.time, depth=depth)
            return self.left
        self.left = KeyNode(chacha = leftcha, time=self.time + [0], depth=depth)
        return self.left
    
    def gen_right_node(self, depth):
        rightcha = ChaCha20_Poly1305.new(self.chacha[len(self.chacha)//2:])
        if (self.time == None): 
            self.time = [1]
            self.right = KeyNode(chacha = rightcha, time=self.time, depth=depth)
            return self.right
        self.right = KeyNode(chacha = rightcha, time=self.time + [1], depth=depth)
        return self.right
    
#TODO: I think this algorithm works to generate the minimum number of nodes.
def generate_tree(node: KeyNode, min_time, max_time, k):
    if min_time == None and max_time == None:
        print("over")
        return

    if (min_time and min_time[-1] == '0') and (max_time and max_time[-1] == '1'):
        if k == 63:
            nodes_arr.append(node)
            return
        print(f"breaking at {k} with {node.time} and {max_time[:-1]}\n" + 
              f"                              {min_time[:-1]}")
        gen_minNode(node.gen_left_node(k), min_time[:-1], k + 1)
        gen_maxNode(node.gen_right_node(k), max_time[:-1], k + 1)
        return
    
    if min_time and min_time[-1] == '0':
        node.gen_left_node(k)
        generate_tree(node.left, min_time[:-1], max_time[:-1], k+1)
    elif max_time and max_time[-1] == '1':
        node.gen_right_node(k)
        generate_tree(node.right, min_time[:-1], max_time[:-1], k+1)
        
def gen_minNode(node: KeyNode, min_time, k):
    k += 1
    if not ('1' in min_time): 
        nodes_arr.append(node)
        return
    if len(min_time) == 1:
        if min_time.pop() == '0':
            nodes_arr.append(node)
        else:
            nodes_arr.append(node.gen_right_node(k))
        return
    if min_time.pop() == '0':
        temp = node.gen_right_node(k)
        nodes_arr.append(temp)
        gen_minNode(node.gen_left_node(k), min_time, k)
    else:
        gen_minNode(node.gen_right_node(k), min_time, k)
    
def gen_maxNode(node: KeyNode, max_time, k):
    k += 1
    if not ('0' in max_time): 
        nodes_arr.append(node)
        return
    if len(max_time) == 1:
        if max_time.pop() == '1':
            nodes_arr.append(node)
        else:
            nodes_arr.append(node.gen_left_node(k))
        return
    if max_time.pop() == '1':
        temp = node.gen_left_node(k)
        nodes_arr.append(temp)
        gen_maxNode(node.gen_right_node(k), max_time, k)
    else: 
        gen_maxNode(node.gen_left_node(k), max_time, k)
    
  
head = KeyNode(chacha=ChaCha20_Poly1305(SOMETHING_HERE)) 
head = KeyNode() 

"""
mint=0
maxt=7

min_time_stack = list(bin(mint)[2:].zfill(64))[::-1]
max_time_stack = list(bin(maxt)[2:].zfill(64))[::-1]
generate_tree(head, min_time_stack, max_time_stack, 0)

print(nodes_arr)
print(len(nodes_arr))
for node in nodes_arr:
    print(f"Time: {node.time}, Depth: {node.depth}, {len(node.time) == 64}")
    
print(f"minimum binary: {min_time_stack[::-1]}")
print(f"maximum binary: {max_time_stack[::-1]}")

#code.interact(local=locals())
"""