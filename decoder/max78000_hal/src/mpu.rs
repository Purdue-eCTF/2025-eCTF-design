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

const MPU_CTRL_ENABLE: u32 = 1;
const MPU_CTRL_HARD_FAULT_ENABLE: u32 = 1 << 1;

/// Represents permisions for mpu region.
#[derive(Debug, Clone, Copy)]
pub struct MpuPerms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

/// Possible sizes for mpu region.
// only some possible values are specified, add more later if more sizes are needed
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

/// Represents caching behavior cpu will use when accessing a regin of memory.
// there are more possible caching behaviors, but we don't need them so they aren't yet added
#[derive(Debug, Clone, Copy)]
pub enum MemoryCacheType {
    StronglyOrdered,
    DeviceShared,
    WriteBackUnshared,
}

impl MemoryCacheType {
    fn make_memory_type_bits(tex: u32, c: u32, b: u32, s: u32) -> u32 {
        (tex & 0b111) << 3 | (s & 1) << 2 | (c & 1) << 1 | (b & 1)
    }

    /// Convert MemoryCacheType to tex, c, b, and s bits in the rasr register.
    fn to_bits(&self) -> u32 {
        match self {
            Self::StronglyOrdered => Self::make_memory_type_bits(0, 0, 0, 0),
            Self::DeviceShared => Self::make_memory_type_bits(0, 0, 1, 0),
            Self::WriteBackUnshared => Self::make_memory_type_bits(0, 1, 1, 0),
        }
    }
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

    /// Contruct base address register, which specifies region number and start address of memory region.
    fn construct_rbar(region_number: u32, base_address: u32) -> u32 {
        // cortex m4 apparently only have 8 slots
        assert!(region_number < 8);
        assert!(base_address & RBAR_ADDR_MASK == base_address);

        (base_address & RBAR_ADDR_MASK)
            | (region_number & RBAR_REGION_MASK)
            | RBAR_ENABLED
    }

    // Construct rasr value, which specfies size of memory region, as well as access permissions and caching behavior.
    fn construct_rasr(size: MpuRegionSize, disable_mask: u8, permissions: MpuPerms, cache_type: MemoryCacheType) -> u32 {
        let execute_disable = if permissions.execute { 0 } else { RASR_EXECUTE_DISABLE };

        let access_perms = match (permissions.read, permissions.write) {
            (false, false) => RASR_NO_ACCESS,
            (true, false) => RASR_RO_PRIVILEGED,
            (_, true) => RASR_RW_PRIVILEGED,
        };

        execute_disable
            | access_perms
            | (cache_type.to_bits() << 16)
            | ((disable_mask as u32) << 8)
            | ((size as u32) << 1)
            | RASR_ENABLED
    }

    /// Set the entry corresponding to `region_number` to have all the specified attributes.
    pub unsafe fn set_region(
        &mut self,
        region_number: u32,
        base_address: u32,
        region_size: MpuRegionSize,
        disable_mask: u8,
        permissions: MpuPerms,
        cache_type: MemoryCacheType,
    ) -> (u32, u32) {
        let rbar = Self::construct_rbar(region_number, base_address);
        let rasr = Self::construct_rasr(region_size, disable_mask, permissions, cache_type);

        unsafe {
            self.set_region_inner(rbar, rasr);
        }

        (rbar, rasr)
    }

    /// Clears the given region number from any memory protections.
    pub unsafe fn clear_region(&mut self, region_number: u32) {
        assert!(region_number < 8);

        unsafe {
            self.set_region_inner(RBAR_ENABLED | region_number, 0);
        }
    }

    /// Enables the MPU.
    pub unsafe fn enable(&mut self) {
        unsafe {
            self.regs.ctrl.write(MPU_CTRL_ENABLE | MPU_CTRL_HARD_FAULT_ENABLE);
        }
    }
}