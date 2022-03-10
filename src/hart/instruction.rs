use core::ops::{Index, IndexMut};

use crate::{
    bus::{Bus, BusError},
    hart::{
        csr::{self, CsrAddress},
        Hart, PrivilegeLevel, TrapCause, TrapValue,
    },
};

use super::InvalidCsr;

/// A register index that is guaranteed to index a valid register (i.e., it is
/// less than 32).
#[derive(Clone, Copy, PartialEq)]
struct RegisterIndex(usize);

pub fn lui<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = u_imm(raw) as u64;
}

pub fn auipc<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.pc.wrapping_add(u_imm(raw) as u64);
}

pub fn jal<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.next;
    let target = hart.pc.wrapping_add(j_imm(raw) as u64);

    if target & 0b11 == 0 {
        hart.next = target
    } else {
        hart.trap(TrapCause::InstructionAddressMisaligned, TrapValue(target))
    }
}

pub fn jalr<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.next;
    let target = hart.gpr[rs1(raw)].wrapping_add(i_imm(raw) as u64) & !0 << 1;

    if target & 0b11 == 0 {
        hart.next = target
    } else {
        hart.trap(TrapCause::InstructionAddressMisaligned, TrapValue(target))
    }
}

pub fn beq<B>(hart: &mut Hart<B>, raw: u32) {
    branch(hart, raw, hart.gpr[rs1(raw)] == hart.gpr[rs2(raw)])
}

pub fn bne<B>(hart: &mut Hart<B>, raw: u32) {
    branch(hart, raw, hart.gpr[rs1(raw)] != hart.gpr[rs2(raw)])
}

pub fn blt<B>(hart: &mut Hart<B>, raw: u32) {
    branch(
        hart,
        raw,
        (hart.gpr[rs1(raw)] as i64) < hart.gpr[rs2(raw)] as i64,
    )
}

pub fn bge<B>(hart: &mut Hart<B>, raw: u32) {
    branch(
        hart,
        raw,
        hart.gpr[rs1(raw)] as i64 >= hart.gpr[rs2(raw)] as i64,
    )
}

pub fn bltu<B>(hart: &mut Hart<B>, raw: u32) {
    branch(hart, raw, hart.gpr[rs1(raw)] < hart.gpr[rs2(raw)])
}

pub fn bgeu<B>(hart: &mut Hart<B>, raw: u32) {
    branch(hart, raw, hart.gpr[rs1(raw)] >= hart.gpr[rs2(raw)])
}

#[inline(always)]
fn branch<B>(hart: &mut Hart<B>, raw: u32, condition: bool) {
    let target = hart.pc.wrapping_add(b_imm(raw) as u64);

    if condition {
        if target & 0b11 == 0 {
            hart.next = target;
        } else {
            hart.trap(TrapCause::InstructionAddressMisaligned, TrapValue(target))
        }
    }
}

#[inline]
fn l<T, B: Bus<u64, T>>(hart: &mut Hart<B>, raw: u32, convert: impl FnOnce(T) -> u64) {
    let address = hart.gpr[rs1(raw)].wrapping_add(i_imm(raw) as u64);

    match hart.bus.load(address) {
        Ok(value) => hart.gpr[rd(raw)] = convert(value),
        Err(BusError::AccessFault) => hart.trap(TrapCause::LoadAccessFault, TrapValue(address)),
        Err(BusError::AddressMisaligned) => {
            hart.trap(TrapCause::LoadAddressMisaligned, TrapValue(address))
        }
    }
}

pub fn lb<B: Bus<u64, u8>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as i8 as u64)
}

pub fn lh<B: Bus<u64, u16>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as i16 as u64)
}

pub fn lw<B: Bus<u64, u32>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as i32 as u64)
}

pub fn ld<B: Bus<u64, u64>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x)
}

pub fn lbu<B: Bus<u64, u8>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as u64)
}

pub fn lhu<B: Bus<u64, u8>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as u64)
}

pub fn lwu<B: Bus<u64, u8>>(hart: &mut Hart<B>, raw: u32) {
    l(hart, raw, |x| x as u64)
}

#[inline]
fn s<T, B: Bus<u64, T>>(hart: &mut Hart<B>, raw: u32, convert: impl FnOnce(u64) -> T) {
    let address = hart.gpr[rs1(raw)].wrapping_add(s_imm(raw) as u64);
    let value = convert(hart.gpr[rs2(raw)]);

    match hart.bus.store(address, value) {
        Ok(()) => (),
        Err(BusError::AccessFault) => hart.trap(TrapCause::StoreAmoAccessFault, TrapValue(address)),
        Err(BusError::AddressMisaligned) => {
            hart.trap(TrapCause::StoreAmoAccessFault, TrapValue(address))
        }
    }
}

pub fn sb<B: Bus<u64, u8>>(hart: &mut Hart<B>, raw: u32) {
    s(hart, raw, |r| r as u8)
}

pub fn sh<B: Bus<u64, u16>>(hart: &mut Hart<B>, raw: u32) {
    s(hart, raw, |r| r as u16)
}

pub fn sw<B: Bus<u64, u32>>(hart: &mut Hart<B>, raw: u32) {
    s(hart, raw, |r| r as u32)
}

pub fn sd<B: Bus<u64, u64>>(hart: &mut Hart<B>, raw: u32) {
    s(hart, raw, |r| r as u64)
}

pub fn addi<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_add(i_imm(raw) as u64);
}

pub fn addiw<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] as u32).wrapping_add(i_imm(raw) as u32) as i32 as u64;
}

pub fn slti<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = ((hart.gpr[rs1(raw)] as i64) < i_imm(raw) as i64) as u64;
}

pub fn sltiu<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] < i_imm(raw) as u64) as u64;
}

pub fn xori<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] ^ i_imm(raw) as u64;
}

pub fn ori<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] | i_imm(raw) as u64;
}

pub fn andi<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] & i_imm(raw) as u64;
}

pub fn slli<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_shl(shamt(raw));
}

pub fn srxi<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // SRLI
        hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_shr(shamt(raw))
    } else {
        // SRAI
        hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] as i64).wrapping_shr(shamt(raw)) as u64
    }
}

pub fn slliw<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] as i32).wrapping_shl(shamt(raw)) as u64;
}

pub fn srxiw<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // SRLIW
        hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] as u32).wrapping_shr(shamt(raw)) as i32 as u64
    } else {
        // SRAIW
        hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] as i32).wrapping_shr(shamt(raw)) as u64
    }
}

pub fn add_sub<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // ADD
        hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_add(hart.gpr[rs2(raw)])
    } else {
        // SUB
        hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_sub(hart.gpr[rs2(raw)])
    }
}

pub fn addw_subw<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // ADDW
        hart.gpr[rd(raw)] =
            (hart.gpr[rs1(raw)] as u32).wrapping_add(hart.gpr[rs2(raw)] as u32) as i32 as u64
    } else {
        // SUBW
        hart.gpr[rd(raw)] =
            (hart.gpr[rs1(raw)] as u32).wrapping_sub(hart.gpr[rs2(raw)] as u32) as i32 as u64
    }
}

pub fn sll<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_shl(hart.gpr[rs2(raw)] as u32);
}

pub fn slt<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = ((hart.gpr[rs1(raw)] as i64) < hart.gpr[rs2(raw)] as i64) as u64;
}

pub fn sltu<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = (hart.gpr[rs1(raw)] < hart.gpr[rs2(raw)]) as u64;
}

pub fn xor<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] ^ hart.gpr[rs2(raw)];
}

pub fn srx<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // SRL
        hart.gpr[rd(raw)] = hart.gpr[rs1(raw)].wrapping_shr(hart.gpr[rs2(raw)] as u32)
    } else {
        // SRA
        hart.gpr[rd(raw)] =
            (hart.gpr[rs1(raw)] as i64).wrapping_shr(hart.gpr[rs2(raw)] as u32) as u64
    }
}

pub fn or<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] | hart.gpr[rs2(raw)];
}

pub fn and<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] = hart.gpr[rs1(raw)] & hart.gpr[rs2(raw)];
}

pub fn sllw<B>(hart: &mut Hart<B>, raw: u32) {
    hart.gpr[rd(raw)] =
        (hart.gpr[rs1(raw)] as u32).wrapping_shl(hart.gpr[rs2(raw)] as u32) as i32 as u64;
}

pub fn srxw<B>(hart: &mut Hart<B>, raw: u32) {
    if raw & 1 << 30 == 0 {
        // SRLW
        hart.gpr[rd(raw)] =
            (hart.gpr[rs1(raw)] as u32).wrapping_shr(hart.gpr[rs2(raw)] as u32) as i32 as u64
    } else {
        // SRAW
        hart.gpr[rd(raw)] =
            (hart.gpr[rs1(raw)] as i32).wrapping_shr(hart.gpr[rs2(raw)] as u32) as u64
    }
}

pub fn fence<B>(_hart: &mut Hart<B>, _raw: u32) {}

pub fn ecall_ebreak<B>(hart: &mut Hart<B>, raw: u32) {
    let cause = if raw & 1 << 20 == 0 {
        match hart.privilege {
            PrivilegeLevel::Machine => TrapCause::EnvironmentCallFromMMode,
            PrivilegeLevel::Supervisor => TrapCause::EnvironmentCallFromSMode,
            PrivilegeLevel::User => TrapCause::EnvironmentCallFromUMode,
        }
    } else {
        TrapCause::Breakpoint
    };

    hart.trap(cause, TrapValue::empty())
}

pub fn csrrw<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if csr::is_read_only(address) || !csr::is_sufficient_privilege(address, hart.privilege) {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    // Save value of rs1 in case it is an alias for rd
    let initial_rs1 = hart.gpr[rs1(raw)];

    let result = hart.csr(address, |_| initial_rs1);

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

pub fn csrrs<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if !csr::is_sufficient_privilege(address, hart.privilege)
        || rs1(raw) != RegisterIndex::X0 && csr::is_read_only(address)
    {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    // Save value of rs1 in case it is an alias for rd
    let initial_rs1 = hart.gpr[rs1(raw)];

    let result = hart.csr(address, |value| value | initial_rs1);

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

pub fn csrrc<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if !csr::is_sufficient_privilege(address, hart.privilege)
        || rs1(raw) != RegisterIndex::X0 && csr::is_read_only(address)
    {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    // Save value of rs1 in case it is an alias for rd
    let initial_rs1 = hart.gpr[rs1(raw)];

    let result = hart.csr(address, |value| value & !initial_rs1);

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

pub fn csrrwi<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if csr::is_read_only(address) || !csr::is_sufficient_privilege(address, hart.privilege) {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    let result = hart.csr(address, |_| uimm(raw) as u64);

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

pub fn csrrsi<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if !csr::is_sufficient_privilege(address, hart.privilege)
        || uimm(raw) != 0 && csr::is_read_only(address)
    {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    let result = hart.csr(address, |value| value | uimm(raw) as u64);

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

pub fn csrrci<B>(hart: &mut Hart<B>, raw: u32) {
    let address = csr(raw);

    if !csr::is_sufficient_privilege(address, hart.privilege)
        || uimm(raw) != 0 && csr::is_read_only(address)
    {
        return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64));
    }

    let result = hart.csr(address, |value| value & !(uimm(raw) as u64));

    match result {
        Ok(value) => hart.gpr[rd(raw)] = value,
        Err(InvalidCsr) => return hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64)),
    }
}

/// Gets the `rd` field of R-, I-, U-, and J-type instructions.
#[inline(always)]
const fn rd(raw: u32) -> RegisterIndex {
    // SAFETY: Since the value is masked with 0b11111, it will always be
    // less than 32.
    unsafe { RegisterIndex::new_unchecked(raw as usize >> 7 & 0b11111) }
}

/// Gets the `rs1` field of R-, I-, S-, and U-type instructions.
#[inline(always)]
const fn rs1(raw: u32) -> RegisterIndex {
    // SAFETY: Since the value is masked with 0b11111, it will always be
    // less than 32.
    unsafe { RegisterIndex::new_unchecked(raw as usize >> 15 & 0b11111) }
}

/// Gets the `rs2` field of R-, S-, and B-type instructions.
#[inline(always)]
const fn rs2(raw: u32) -> RegisterIndex {
    // SAFETY: Since the value is masked with 0b11111, it will always be
    // less than 32.
    unsafe { RegisterIndex::new_unchecked(raw as usize >> 20 & 0b11111) }
}

/// Gets the `imm` field of I-type instructions.
#[inline(always)]
const fn i_imm(raw: u32) -> i32 {
    raw as i32 >> 20
}

/// Gets the `imm` field of S-type instructions.
#[inline(always)]
const fn s_imm(raw: u32) -> i32 {
    // imm[4:0]
    raw as i32 >> 7 & 0b11111
    // imm[11:5]
        | raw as i32 >> 20 & !0 << 5
}

/// Gets the `imm` field of B-type instructions.
#[inline(always)]
const fn b_imm(raw: u32) -> i32 {
    // imm[4:1]
    (raw >> 7 & 0b11110) as i32
    // imm[10:5] + imm[12]
        | raw as i32 >> 20 & (!0b11111 & !(1 << 11))
    // imm[11]
        | (raw << 4 & 1 << 11) as i32
}

/// Gets the `imm` field of U-type instructions.
#[inline(always)]
const fn u_imm(raw: u32) -> i32 {
    // imm[31:12]
    raw as i32 & !0 << 12
}

/// Gets the `imm` field of J-type instructions.
#[inline(always)]
const fn j_imm(raw: u32) -> i32 {
    // imm[20] + imm[19:12] + imm[10:1]
    raw as i32 >> 20 & (!0 << 1 & !(1 << 11))
    // imm[11]
        | (raw >> 9 & 1 << 11) as i32
}

/// Gets the `shamt` field of shift instructions.
#[inline(always)]
const fn shamt(raw: u32) -> u32 {
    raw >> 20 & 0b111111
}

/// Gets the `csr` field of CSR instructions.
#[inline(always)]
fn csr(raw: u32) -> CsrAddress {
    // SAFETY: & 0xFFF guarantees that the value passed to `new` is less than
    // 4096.
    unsafe { CsrAddress::new_unchecked((raw >> 20 & 0xFFF) as u16) }
}

/// Gets the `uimm` field of immediate CSR instructions.
#[inline(always)]
const fn uimm(raw: u32) -> u32 {
    raw >> 15 & 0b11111
}

impl Index<RegisterIndex> for [u64; 32] {
    type Output = u64;

    fn index(&self, index: RegisterIndex) -> &u64 {
        // SAFETY: index.0 is guaranteed to be less than 32, so this access is
        // always valid.
        unsafe { self.get_unchecked(index.0) }
    }
}

impl IndexMut<RegisterIndex> for [u64; 32] {
    fn index_mut(&mut self, index: RegisterIndex) -> &mut u64 {
        // SAFETY: index.0 is guaranteed to be less than 32, so this access is
        // always valid.
        unsafe { self.get_unchecked_mut(index.0) }
    }
}

impl RegisterIndex {
    /// The `zero` register.
    pub const X0: RegisterIndex = RegisterIndex(0);

    /// Creates a register from a register index; i.e. `index` = 0 corresponds
    /// to x0, `index` = 1 to x1, etc.
    ///
    /// # Safety
    /// `index` must lie in the range 0..=31.
    pub const unsafe fn new_unchecked(index: usize) -> RegisterIndex {
        RegisterIndex(index)
    }
}
