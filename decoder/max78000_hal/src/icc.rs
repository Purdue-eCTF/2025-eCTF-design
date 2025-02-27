use max78000_device::ICC0;

pub struct Icc {
    regs: ICC0,
}

impl Icc {
    pub(crate) fn new(regs: ICC0) -> Self {
        Icc { regs }
    }

    fn is_ready(&self) -> bool {
        self.regs.ctrl().read().rdy().is_ready()
    }

    fn invalidate(&mut self) {
        // safety: setting bit to 1 corresponds to invalidating the cache
        self.regs.invalidate().write(|invalidate| unsafe {
            invalidate.invalid().bits(1)
        });

        while !self.is_ready() {}
    }

    pub fn enable(&mut self) {
        // follow same procedure as UIUC hal
        self.disable();
        self.invalidate();

        self.regs.ctrl().modify(|_, ctrl| ctrl.en().en());
        while !self.is_ready() {}
    }

    pub fn disable(&mut self) {
        self.regs.ctrl().modify(|_, ctrl| ctrl.en().dis());
    }
}