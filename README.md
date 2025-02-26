# Purdue ECTF 2025
This repository contains all code related to Purdue3's (b01lers)  submission for ECTF 2025. 

## High-level overview
This project is a secure implementation of an embedded system for the MITRE eCTF challenge. Contestents were tasked with creating a system
to ensure the secure transmission of television frames through a satellite television distribution system. Teams were tasked with creating
an `encoder` to properly encode non-encrypted frames as well as a `decoder` to decrypt these frames. Frames are sent from the encoder to an 
uplink where they are then relayed to an organizer controlled satellite. The satellite broadcasts frames to all listening TVs; each teams' 
decoder is hooked up to a TV where it will accept encoded frames and then decode them if authorized to do so. 

## Documentation 📖
Documentation and technical specifications can be found in the [docs](docs) directory.

## Design Structure
 - `decoder` - Contains the source code for the secure decoder implementation.
 - `design` - Contains the encoder and necessary secret generation scripts. 
 - `tools` - Contains organizer created host tools that facilitate interaction between all components of the embedded system.  

## Team Members 👥
**Students**: Nick Andry, William Boulton, Philip Frey,  Neil Van Eikema Hommes, Jihun (Jimmy) Hwang, Jaxson Pahukula, Jack Reynolds, Jack Roscoe, Gabe Samide, Lucas Tan, 
Vinh Pham Ngoc Thanh, Vivan Tiwari, Sebastian Toro, Jacob White, Larry Xue, Bronson Yen,  Kevin Yu

**Advisors**: Christina Garman, Santiago Torres-Arias
