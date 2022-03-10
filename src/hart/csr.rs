//! CSR management.

use super::{PrivilegeLevel, TrapCause, TrapValue};

/// Checks if the given privilege level is sufficient to access the CSR at the
/// given address.
pub fn is_sufficient_privilege(address: CsrAddress, privilege: PrivilegeLevel) -> bool {
    address.inner() >> 8 & 0b11 >= privilege as u16
}

/// Checks if a CSR address is read-only.
pub fn is_read_only(address: CsrAddress) -> bool {
    address.inner() & 0b11 << 10 == 0b11 << 10
}

/// This struct contains a CSR address which is guaranteed to be less than or equal to 4096.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CsrAddress(u16);

impl CsrAddress {
    pub const SSTATUS: CsrAddress = CsrAddress::new(0x100);
    pub const MSTATUS: CsrAddress = CsrAddress::new(0x300);
    pub const MISA: CsrAddress = CsrAddress::new(0x301);
    pub const MEDELEG: CsrAddress = CsrAddress::new(0x302);
    pub const SIE: CsrAddress = CsrAddress::new(0x104);
    pub const MIE: CsrAddress = CsrAddress::new(0x304);
    pub const STVEC: CsrAddress = CsrAddress::new(0x105);
    pub const MTVEC: CsrAddress = CsrAddress::new(0x305);
    pub const SCOUNTEREN: CsrAddress = CsrAddress::new(0x106);
    pub const MCOUNTEREN: CsrAddress = CsrAddress::new(0x306);
    pub const SSCRATCH: CsrAddress = CsrAddress::new(0x140);
    pub const MSCRATCH: CsrAddress = CsrAddress::new(0x340);
    pub const SEPC: CsrAddress = CsrAddress::new(0x141);
    pub const MEPC: CsrAddress = CsrAddress::new(0x341);
    pub const SCAUSE: CsrAddress = CsrAddress::new(0x142);
    pub const MCAUSE: CsrAddress = CsrAddress::new(0x342);
    pub const STVAL: CsrAddress = CsrAddress::new(0x143);
    pub const MTVAL: CsrAddress = CsrAddress::new(0x343);
    pub const SIP: CsrAddress = CsrAddress::new(0x144);
    pub const MIP: CsrAddress = CsrAddress::new(0x344);
    pub const SATP: CsrAddress = CsrAddress::new(0x180);
    pub const MVENDORID: CsrAddress = CsrAddress::new(0xF11);
    pub const MARCHID: CsrAddress = CsrAddress::new(0xF12);
    pub const MIMPID: CsrAddress = CsrAddress::new(0xF13);
    pub const MHARTID: CsrAddress = CsrAddress::new(0xF14);

    /// Creates a new CsrAddress, panicking if it does not fit into 12 bits.
    pub const fn new(address: u16) -> CsrAddress {
        if address <= 0xFFF {
            // SAFETY: We have checked the constraint of `new_unchecked`.
            unsafe { CsrAddress::new_unchecked(address) }
        } else {
            panic!("CSR address must be less than 0xFFF")
        }
    }

    /// Creates a new CsrAddress without checking if the address is valid.
    ///
    /// # Safety
    /// `address` must be less than or equal to `0xFFF`.
    pub const unsafe fn new_unchecked(address: u16) -> CsrAddress {
        CsrAddress(address)
    }

    pub const fn inner(self) -> u16 {
        self.0
    }
}

pub struct Csr {
    pub status: Status,
    pub medeleg: Deleg,
    pub mideleg: Deleg,
    pub sie: Ie,
    pub mie: Ie,
    pub stvec: Tvec,
    pub mtvec: Tvec,
    pub sscratch: Scratch,
    pub mscratch: Scratch,
    pub sepc: Epc,
    pub mepc: Epc,
    pub scause: Cause,
    pub mcause: Cause,
    pub stval: Tval,
    pub mtval: Tval,
    pub sip: Ip,
    pub mip: Ip,
    pub mvendorid: ReadOnly,
    pub marchid: ReadOnly,
    pub mimpid: ReadOnly,
    pub mhartid: ReadOnly,
    // ...
}

pub struct ReadOnly(pub u64);

impl ReadOnly {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        f(self.0);
        self.0
    }

    pub fn set(&mut self, value: u64) {
        self.0 = value
    }
}

pub struct Ip(pub u64);

impl Ip {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        // TODO: implement properly: mask out non-existent interrupts
        self.0 = f(self.0);
        self.0
    }

    pub fn is_pending(&self, interrupt: TrapCause) -> bool {
        self.0.wrapping_shr(interrupt as u32) & 0b1 != 0
    }

    pub fn set_pending(&mut self, interrupt: TrapCause, value: bool) {
        let mask = !1u64.wrapping_shr(interrupt as u32);
        let value = (value as u64).wrapping_shl(interrupt as u32);
        self.0 = self.0 & mask | value
    }
}

pub struct Ie(pub u64);

impl Ie {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        // TODO: implement properly: mask out non-existent interrupts
        self.0 = f(self.0);
        self.0
    }

    pub fn is_enabled(&self, interrupt: TrapCause) -> bool {
        self.0.wrapping_shr(interrupt as u32) & 0b1 != 0
    }
}

pub struct Tval(pub u64);

impl Tval {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        self.0 = f(self.0);
        self.0
    }

    pub fn set(&mut self, value: TrapValue) {
        self.0 = value.0
    }
}

pub struct Cause(pub u64);

impl Cause {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        // TODO: implement correctly: should only allow valid causes
        self.0 = f(self.0);
        self.0
    }

    pub fn set(&mut self, cause: TrapCause) {
        self.0 = cause as u64
    }
}

pub struct Epc(pub u64);

impl Epc {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        self.0 = f(self.0);
        self.0
    }

    pub fn set(&mut self, epc: u64) {
        self.0 = epc
    }
}

pub struct Scratch(pub u64);

impl Scratch {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        self.0 = f(self.0);
        self.0
    }
}

pub struct Tvec(pub u64);

impl Tvec {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        // masks out upper bit of mode to only allow mode = 0, 1
        self.0 = f(self.0) & !0b10;
        self.0
    }

    pub fn mode(&self) -> u64 {
        self.0 & 0b11
    }

    pub fn base(&self) -> u64 {
        self.0 & !0b11
    }
}

pub struct Deleg(pub u64);

impl Deleg {
    pub fn access(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        // TODO: implement correctly: only allow valid interrupts/exceptions
        self.0 = f(self.0);
        self.0
    }

    pub fn is_delegated(&self, trap: TrapCause) -> bool {
        self.0.wrapping_shr(trap as u32) & 0b1 != 0
    }
}

pub struct Status(pub u64);

impl Status {
    const M_WRITE_MASK: u64 = 0b1 << 1
        | 0b1 << 3
        | 0b1 << 5
        | 0b1 << 7
        | 0b1 << 8
        | 0b1 << 17
        | 0b1 << 18
        | 0b1 << 19
        | 0b1 << 20
        | 0b1 << 21
        | 0b1 << 22;
    const S_WRITE_MASK: u64 = 0b1 << 1
        | 0b1 << 5
        | 0b1 << 8
        | 0b1 << 18
        | 0b1 << 19;
    const S_READ_MASK: u64 = Self::S_WRITE_MASK;

    pub fn access_mstatus(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        self.0 = self.0 & !Self::M_WRITE_MASK | f(self.0) & Self::M_WRITE_MASK;
        self.0
    }

    pub fn access_sstatus(&mut self, f: impl FnOnce(u64) -> u64) -> u64 {
        self.0 = self.0 & !Self::S_WRITE_MASK | f(self.0) & Self::S_WRITE_MASK;
        self.0 & Self::S_READ_MASK
    }

    /// Supervisor interrupt enable.
    pub fn sie(&self) -> u64 {
        self.0 >> 1 & 0b1
    }

    /// Machine interrupt enable.
    pub fn mie(&self) -> u64 {
        self.0 >> 3 & 0b1
    }

    /// Supervisor previous interrupt enable.
    pub fn spie(&self) -> u64 {
        self.0 >> 5 & 0b1
    }

    /// User big endian.
    pub fn ube(&self) -> u64 {
        self.0 >> 6 & 0b1
    }

    /// Machine previous interrupt enable.
    pub fn mpie(&self) -> u64 {
        self.0 >> 7 & 0b1
    }

    /// Supervisor previous privilege.
    pub fn spp(&self) -> u64 {
        self.0 >> 8 & 0b1
    }

    /// Vector extension state.
    pub fn vs(&self) -> u64 {
        self.0 >> 9 & 0b11
    }

    /// Machine previous privilege.
    pub fn mpp(&self) -> u64 {
        self.0 >> 11 & 0b11
    }

    /// Floating-point extension state.
    pub fn fs(&self) -> u64 {
        self.0 >> 13 & 0b11
    }

    /// User-mode extensions and associated state.
    pub fn xs(&self) -> u64 {
        self.0 >> 15 & 0b11
    }

    /// Modify privilege.
    pub fn mprv(&self) -> u64 {
        self.0 >> 17 & 0b1
    }

    /// Supervisor user memory.
    pub fn sum(&self) -> u64 {
        self.0 >> 18 & 0b1
    }

    /// Make executable readable.
    pub fn mxr(&self) -> u64 {
        self.0 >> 19 & 0b1
    }

    /// Trap virtual memory.
    pub fn tvm(&self) -> u64 {
        self.0 >> 20 & 0b1
    }

    /// Timeout wait.
    pub fn tw(&self) -> u64 {
        self.0 >> 21 & 0b1
    }

    /// Trap SRET.
    pub fn tsr(&self) -> u64 {
        self.0 >> 22 & 0b1
    }

    /// User XLEN.
    pub fn uxl(&self) -> u64 {
        self.0 >> 32 & 0b11
    }

    /// Supervisor XLEN.
    pub fn sxl(&self) -> u64 {
        self.0 >> 34 & 0b11
    }

    /// Supervisor big endian.
    pub fn sbe(&self) -> u64 {
        self.0 >> 36 & 0b1
    }

    /// Machine big endian.
    pub fn mbe(&self) -> u64 {
        self.0 >> 37 & 0b1
    }

    /// Summarized dirtiness.
    pub fn sd(&self) -> u64 {
        self.0 >> 63 & 0b1
    }
}
