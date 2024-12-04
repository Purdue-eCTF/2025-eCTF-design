# @file list_tool.py
# @author Frederich Stine
# @brief host tool for listing installed components 
# @date 2024
#
# This source file is part of an example system for MITRE's 2024 Embedded CTF (eCTF).
# This code is being provided only for educational purposes for the 2024 MITRE eCTF
# competition, and may not meet MITRE standards for quality. Use this code at your
# own risk!
#
# @copyright Copyright (c) 2024 The MITRE Corporation

import argparse
import serial


# List function
def listen(args):
    ser = serial.Serial(
        port=args.application_processor,
        baudrate=115200,
        parity=serial.PARITY_NONE,
        stopbits=serial.STOPBITS_ONE,
        bytesize=serial.EIGHTBITS,
    )

    # Receive messages until done
    while True:
        byte = ser.read()
        char = byte.decode("utf-8")
        print(char, end='')

# Main function
def main():
    parser = argparse.ArgumentParser(
        prog="eCTF List Host Tool",
        description="List the components connected to the medical device",
    )

    parser.add_argument(
        "-a", "--application-processor", required=True, help="Serial device of the AP"
    )

    args = parser.parse_args()

    listen(args)


if __name__ == "__main__":
    main()

