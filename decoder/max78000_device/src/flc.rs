#[doc = r"Register block"]
#[repr(C)]
pub struct RegisterBlock {
    addr: ADDR,
    clkdiv: CLKDIV,
    ctrl: CTRL,
    _reserved3: [u8; 0x18],
    intr: INTR,
    eccdata: ECCDATA,
    _reserved5: [u8; 0x04],
    data: [DATA; 4],
    actrl: ACTRL,
    _reserved7: [u8; 0x3c],
    welr0: WELR0,
    _reserved8: [u8; 0x04],
    welr1: WELR1,
    _reserved9: [u8; 0x04],
    rlr0: RLR0,
    _reserved10: [u8; 0x04],
    rlr1: RLR1,
}
impl RegisterBlock {
    #[doc = "0x00 - Flash Write Address."]
    #[inline(always)]
    pub const fn addr(&self) -> &ADDR {
        &self.addr
    }
    #[doc = "0x04 - Flash Clock Divide. The clock (PLL0) is divided by this value to generate a 1 MHz clock for Flash controller."]
    #[inline(always)]
    pub const fn clkdiv(&self) -> &CLKDIV {
        &self.clkdiv
    }
    #[doc = "0x08 - Flash Control Register."]
    #[inline(always)]
    pub const fn ctrl(&self) -> &CTRL {
        &self.ctrl
    }
    #[doc = "0x24 - Flash Interrupt Register."]
    #[inline(always)]
    pub const fn intr(&self) -> &INTR {
        &self.intr
    }
    #[doc = "0x28 - ECC Data Register."]
    #[inline(always)]
    pub const fn eccdata(&self) -> &ECCDATA {
        &self.eccdata
    }
    #[doc = "0x30..0x40 - Flash Write Data."]
    #[inline(always)]
    pub const fn data(&self, n: usize) -> &DATA {
        &self.data[n]
    }
    #[doc = "Iterator for array of:"]
    #[doc = "0x30..0x40 - Flash Write Data."]
    #[inline(always)]
    pub fn data_iter(&self) -> impl Iterator<Item = &DATA> {
        self.data.iter()
    }
    #[doc = "0x40 - Access Control Register. Writing the ACTRL register with the following values in the order shown, allows read and write access to the system and user Information block: pflc-actrl = 0x3a7f5ca3; pflc-actrl = 0xa1e34f20; pflc-actrl = 0x9608b2c1. When unlocked, a write of any word will disable access to system and user information block. Readback of this register is always zero."]
    #[inline(always)]
    pub const fn actrl(&self) -> &ACTRL {
        &self.actrl
    }
    #[doc = "0x80 - WELR0"]
    #[inline(always)]
    pub const fn welr0(&self) -> &WELR0 {
        &self.welr0
    }
    #[doc = "0x88 - WELR1"]
    #[inline(always)]
    pub const fn welr1(&self) -> &WELR1 {
        &self.welr1
    }
    #[doc = "0x90 - RLR0"]
    #[inline(always)]
    pub const fn rlr0(&self) -> &RLR0 {
        &self.rlr0
    }
    #[doc = "0x98 - RLR1"]
    #[inline(always)]
    pub const fn rlr1(&self) -> &RLR1 {
        &self.rlr1
    }
}
#[doc = "ADDR (rw) register accessor: Flash Write Address.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`addr::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`addr::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@addr`]
module"]
pub type ADDR = crate::Reg<addr::ADDR_SPEC>;
#[doc = "Flash Write Address."]
pub mod addr;
#[doc = "CLKDIV (rw) register accessor: Flash Clock Divide. The clock (PLL0) is divided by this value to generate a 1 MHz clock for Flash controller.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`clkdiv::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`clkdiv::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@clkdiv`]
module"]
pub type CLKDIV = crate::Reg<clkdiv::CLKDIV_SPEC>;
#[doc = "Flash Clock Divide. The clock (PLL0) is divided by this value to generate a 1 MHz clock for Flash controller."]
pub mod clkdiv;
#[doc = "CTRL (rw) register accessor: Flash Control Register.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`ctrl::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`ctrl::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@ctrl`]
module"]
pub type CTRL = crate::Reg<ctrl::CTRL_SPEC>;
#[doc = "Flash Control Register."]
pub mod ctrl;
#[doc = "INTR (rw) register accessor: Flash Interrupt Register.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`intr::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`intr::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@intr`]
module"]
pub type INTR = crate::Reg<intr::INTR_SPEC>;
#[doc = "Flash Interrupt Register."]
pub mod intr;
#[doc = "ECCDATA (rw) register accessor: ECC Data Register.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`eccdata::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`eccdata::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@eccdata`]
module"]
pub type ECCDATA = crate::Reg<eccdata::ECCDATA_SPEC>;
#[doc = "ECC Data Register."]
pub mod eccdata;
#[doc = "DATA (rw) register accessor: Flash Write Data.\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`data::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`data::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@data`]
module"]
pub type DATA = crate::Reg<data::DATA_SPEC>;
#[doc = "Flash Write Data."]
pub mod data;
#[doc = "ACTRL (w) register accessor: Access Control Register. Writing the ACTRL register with the following values in the order shown, allows read and write access to the system and user Information block: pflc-actrl = 0x3a7f5ca3; pflc-actrl = 0xa1e34f20; pflc-actrl = 0x9608b2c1. When unlocked, a write of any word will disable access to system and user information block. Readback of this register is always zero.\n\nYou can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`actrl::W`]. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@actrl`]
module"]
pub type ACTRL = crate::Reg<actrl::ACTRL_SPEC>;
#[doc = "Access Control Register. Writing the ACTRL register with the following values in the order shown, allows read and write access to the system and user Information block: pflc-actrl = 0x3a7f5ca3; pflc-actrl = 0xa1e34f20; pflc-actrl = 0x9608b2c1. When unlocked, a write of any word will disable access to system and user information block. Readback of this register is always zero."]
pub mod actrl;
#[doc = "WELR0 (rw) register accessor: WELR0\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`welr0::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`welr0::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@welr0`]
module"]
pub type WELR0 = crate::Reg<welr0::WELR0_SPEC>;
#[doc = "WELR0"]
pub mod welr0;
#[doc = "WELR1 (rw) register accessor: WELR1\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`welr1::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`welr1::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@welr1`]
module"]
pub type WELR1 = crate::Reg<welr1::WELR1_SPEC>;
#[doc = "WELR1"]
pub mod welr1;
#[doc = "RLR0 (rw) register accessor: RLR0\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`rlr0::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`rlr0::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rlr0`]
module"]
pub type RLR0 = crate::Reg<rlr0::RLR0_SPEC>;
#[doc = "RLR0"]
pub mod rlr0;
#[doc = "RLR1 (rw) register accessor: RLR1\n\nYou can [`read`](crate::generic::Reg::read) this register and get [`rlr1::R`].  You can [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`write_with_zero`](crate::generic::Reg::write_with_zero) this register using [`rlr1::W`]. You can also [`modify`](crate::generic::Reg::modify) this register. See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [`mod@rlr1`]
module"]
pub type RLR1 = crate::Reg<rlr1::RLR1_SPEC>;
#[doc = "RLR1"]
pub mod rlr1;
