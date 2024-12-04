use core::mem::size_of;

use max78000_device::TRNG;

use crate::gcr::Gcr;

pub struct Trng {
    regs: TRNG,
}

impl Trng {
    pub(crate) fn new(regs: TRNG) -> Self {
        Gcr::with(|gcr| {
            gcr.set_trng_clock_enabled(true);
        });

        Trng {
            regs,
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        while !self.regs.status().read().rdy().bit() {
            core::hint::spin_loop();
        }

        self.regs.data().read().bits()
    }

    pub fn rand_bytes(&mut self, data: &mut [u8]) {
        for chunk in data.chunks_mut(size_of::<u32>()) {
            let n = self.next_u32();
            chunk.copy_from_slice(&n.to_be_bytes()[..chunk.len()]);
        }
    }

    pub fn gen_nonce<const N: usize>(&mut self) -> [u8; N] {
        let mut nonce = [0; N];
        self.rand_bytes(&mut nonce);
        nonce
    }
}
