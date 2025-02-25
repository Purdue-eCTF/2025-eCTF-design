use cortex_m::peripheral::MPU;

// top RBAR bits are address
// bottom 4 are slot number
// 5th bit is enabled bit
const RBAR_ADDR_MASK: u32 = 0xffffffe0;
const RBAR_REGION_MASK: u32 = 0xf;
const RBAR_ENABLED: u32 = 0x10;

const RASR_EXECUTE_DISABLE: u32 = 1 << 28;
const RASR_NO_ACCESS: u32 = 0 << 24;
const RASR_RW_PRIVILEGED: u32 = 1 << 24;
const RASR_RO_PRIVILEGED: u32 = 0b101 << 24;
const RASR_ENABLED: u32 = 1;

// not exactly sure if this is right or not
// TODO: verify which mode to use
//const UNCACHED_SHARED: u32 = 0b100100 << 16;
const UNCACHED_SHARED: u32 = 0b000100 << 16;

const MPU_CTRL_ENABLE: u32 = 1;
const MPU_CTRL_HARD_FAULT_ENABLE: u32 = 1 << 1;

/// Represents permisions for mpu region
#[derive(Debug, Clone, Copy)]
pub struct MpuPerms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

/// Possible sizes for mpu region
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum MpuRegionSize {
    Bytes32 = 0x4,
    Bytes64 = 0x5,
    Bytes128 = 0x6,
    KibiByte128 = 0x10,
    KibiByte512 = 0x12,
    MibiByte512 = 0x1c,
}

/// Memory protection unit
pub struct Mpu {
    regs: MPU,
}

impl Mpu {
    pub(crate) fn new(regs: MPU) -> Self {
        Mpu { regs }
    }

    /// Sets the memory region by setting base register and size register
    unsafe fn set_region_inner(&mut self, rbar: u32, rasr: u32) {
        unsafe {
            self.regs.rbar.write(rbar);
            self.regs.rasr.write(rasr);
        }
    }

    fn construct_rbar(region_number: u32, base_address: u32) -> u32 {
        // cortex m4 apparently only have 8 slots
        assert!(region_number < 8);
        assert!(base_address & RBAR_ADDR_MASK == base_address);

        (base_address & RBAR_ADDR_MASK)
            | (region_number & RBAR_REGION_MASK)
            | RBAR_ENABLED
    }

    fn construct_rasr(size: MpuRegionSize, disable_mask: u8, permissions: MpuPerms) -> u32 {
        let execute_disable = if permissions.execute { 0 } else { RASR_EXECUTE_DISABLE };

        let access_perms = match (permissions.read, permissions.write) {
            (false, false) => RASR_NO_ACCESS,
            (true, false) => RASR_RO_PRIVILEGED,
            (_, true) => RASR_RW_PRIVILEGED,
        };

        execute_disable
            | access_perms
            | UNCACHED_SHARED
            | ((disable_mask as u32) << 8)
            | ((size as u32) << 1)
            | RASR_ENABLED
    }

    pub unsafe fn set_region(
        &mut self,
        region_number: u32,
        base_address: u32,
        region_size: MpuRegionSize,
        disable_mask: u8,
        permissions: MpuPerms,
    ) -> (u32, u32) {
        let rbar = Self::construct_rbar(region_number, base_address);
        let rasr = Self::construct_rasr(region_size, disable_mask, permissions);

        unsafe {
            self.set_region_inner(rbar, rasr);
        }

        (rbar, rasr)
    }

    pub unsafe fn clear_region(&mut self, region_number: u32) {
        assert!(region_number < 8);

        unsafe {
            self.set_region_inner(RBAR_ENABLED | region_number, 0);
        }
    }

    pub unsafe fn enable(&mut self) {
        unsafe {
            self.regs.ctrl.write(MPU_CTRL_ENABLE | MPU_CTRL_HARD_FAULT_ENABLE);
        }
    }
}