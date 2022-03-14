use crate::bus::{Bus, BusError};

mod csr;
mod instruction;

use csr::*;

/// A simple implementation of a processor that implements only machine mode.
pub struct Hart<B> {
    /// The hart's current privilege level.
    privilege: PrivilegeLevel,
    /// The system bus for reading and writing physical memory addresses.
    bus: B,
    /// The address of the currently-executing instruction, if one is being executed.
    pc: u64,
    /// The address of the next instruction.
    next: u64,
    /// The general-purpose registers x0 through x31.
    gpr: [u64; 32],
    /// The state of all control and status registers.
    csr: Csr,
}

/// All defined RISC-V privilege levels.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    /// User/Application level.
    User = 0b00,
    /// Supervisor level.
    Supervisor = 0b01,
    /// Machine level.
    Machine = 0b11,
}

/// The cause of a trap. This represents the value that gets stored in the `_cause` register.
#[repr(u64)]
#[derive(Clone, Copy)]
enum TrapCause {
    // Interrupts
    UserSoftwareInterrupt = Self::interrupt(0),
    SupervisorSoftwareInterrupt = Self::interrupt(1),
    MachineSoftwareInterrupt = Self::interrupt(2),
    UserTimerInterrupt = Self::interrupt(4),
    SupervisorTimerInterrupt = Self::interrupt(5),
    MachineTimerInterrupt = Self::interrupt(7),
    UserExternalInterrupt = Self::interrupt(8),
    SupervisorExternalInterrupt = Self::interrupt(9),
    MachineExternalInterrupt = Self::interrupt(11),

    // Exceptions
    InstructionAddressMisaligned = Self::exception(0),
    InstructionAccessFault = Self::exception(1),
    IllegalInstruction = Self::exception(2),
    Breakpoint = Self::exception(3),
    LoadAddressMisaligned = Self::exception(4),
    LoadAccessFault = Self::exception(5),
    StoreAmoAddressMisaligned = Self::exception(6),
    StoreAmoAccessFault = Self::exception(7),
    EnvironmentCallFromUMode = Self::exception(8),
    EnvironmentCallFromSMode = Self::exception(9),
    EnvironmentCallFromMMode = Self::exception(11),
    InstructionPageFault = Self::exception(12),
    LoadPageFault = Self::exception(13),
    StoreAmoPageFault = Self::exception(15),
}

/// The value associated with a trap that is written into the `_tval` register.
struct TrapValue(u64);

/// An invalid CSR.
struct InvalidCsr;

impl<B> Hart<B> {
    /// Creates a new `Hart` that connects to `bus` for memory accesses.
    pub fn new(bus: B) -> Hart<B> {
        Hart {
            privilege: PrivilegeLevel::Machine,
            bus,
            pc: 0,
            next: 0,
            gpr: [0; 32],
            csr: Csr {
                status: csr::Status(0),
                medeleg: csr::Deleg(0),
                mideleg: csr::Deleg(0),
                sie: csr::Ie(0),
                mie: csr::Ie(0),
                stvec: csr::Tvec(0),
                mtvec: csr::Tvec(0),
                sscratch: csr::Scratch(0),
                mscratch: csr::Scratch(0),
                sepc: csr::Epc(0),
                mepc: csr::Epc(0),
                scause: csr::Cause(0),
                mcause: csr::Cause(0),
                stval: csr::Tval(0),
                mtval: csr::Tval(0),
                sip: csr::Ip(0),
                mip: csr::Ip(0),
                mvendorid: csr::ReadOnly(0),
                marchid: csr::ReadOnly(0),
                mimpid: csr::ReadOnly(0),
                mhartid: csr::ReadOnly(0),
            },
        }
    }

    /// Executes one instruction at the given address, returning the offset
    /// that should be applied to the PC to execute the next instruction.
    ///
    /// The address may have any alignment; only PC offsets are checked for
    /// alignment.
    pub fn execute(&mut self)
    where
        B: Bus<u64, u64> + Bus<u64, u32> + Bus<u64, u16> + Bus<u64, u8>,
    {
        // Fetch
        let raw: u32 = match self.bus.load(self.pc) {
            Ok(raw) => raw,
            Err(BusError::AccessFault) => {
                return self.trap(TrapCause::InstructionAccessFault, TrapValue(self.pc))
            }
            Err(BusError::AddressMisaligned) => {
                return self.trap(TrapCause::InstructionAddressMisaligned, TrapValue(self.pc))
            }
        };

        // Decode the part that will be matched on
        let funct3_opcode = raw & 0b1111111 | raw >> 5 & 0b111 << 7;

        // Set x0 back to zero in case it was set by a previous instruction
        self.gpr[0] = 0;

        // Calculate the address of the next instruction.
        self.next = self.pc.wrapping_add(4);

        // Match on the opcode (and funct3) to decode the rest of the
        // instruction and execute it
        let instruction = match funct3_opcode {
            0b000_0110111 | 0b001_0110111 | 0b010_0110111 | 0b011_0110111 | 0b100_0110111
            | 0b101_0110111 | 0b110_0110111 | 0b111_0110111 => instruction::lui,
            0b000_0010111 | 0b001_0010111 | 0b010_0010111 | 0b011_0010111 | 0b100_0010111
            | 0b101_0010111 | 0b110_0010111 | 0b111_0010111 => instruction::auipc,
            0b000_1101111 | 0b001_1101111 | 0b010_1101111 | 0b011_1101111 | 0b100_1101111
            | 0b101_1101111 | 0b110_1101111 | 0b111_1101111 => instruction::jal,
            0b000_1100111 => instruction::jalr,
            0b000_1100011 => instruction::beq,
            0b001_1100011 => instruction::bne,
            0b100_1100011 => instruction::blt,
            0b101_1100011 => instruction::bge,
            0b110_1100011 => instruction::bltu,
            0b111_1100011 => instruction::bgeu,
            0b000_0000011 => instruction::lb,
            0b001_0000011 => instruction::lh,
            0b010_0000011 => instruction::lw,
            0b011_0000011 => instruction::ld,
            0b100_0000011 => instruction::lbu,
            0b101_0000011 => instruction::lhu,
            0b110_0000011 => instruction::lwu,
            0b000_0100011 => instruction::sb,
            0b001_0100011 => instruction::sh,
            0b010_0100011 => instruction::sw,
            0b011_0100011 => instruction::sd,
            0b000_0010011 => instruction::addi,
            0b000_0011011 => instruction::addiw,
            0b010_0010011 => instruction::slti,
            0b011_0010011 => instruction::sltiu,
            0b100_0010011 => instruction::xori,
            0b110_0010011 => instruction::ori,
            0b111_0010011 => instruction::andi,
            0b001_0010011 => instruction::slli,
            0b101_0010011 => instruction::srxi,
            0b001_0011011 => instruction::slliw,
            0b101_0011011 => instruction::srxiw,
            0b000_0110011 => instruction::add_sub,
            0b000_0111011 => instruction::addw_subw,
            0b001_0110011 => instruction::sll,
            0b010_0110011 => instruction::slt,
            0b011_0110011 => instruction::sltu,
            0b100_0110011 => instruction::xor,
            0b101_0110011 => instruction::srx,
            0b110_0110011 => instruction::or,
            0b111_0110011 => instruction::and,
            0b001_0111011 => instruction::sllw,
            0b101_0111011 => instruction::srxw,
            0b000_0001111 => instruction::fence,
            0b000_1110011 => instruction::ecall_ebreak,
            0b001_1110011 => instruction::csrrw,
            0b010_1110011 => instruction::csrrs,
            0b011_1110011 => instruction::csrrc,
            0b101_1110011 => instruction::csrrwi,
            0b110_1110011 => instruction::csrrsi,
            0b111_1110011 => instruction::csrrci,
            _ => |hart: &mut Hart<B>, raw| {
                hart.trap(TrapCause::IllegalInstruction, TrapValue(raw as u64))
            },
        };

        instruction(self, raw);

        self.pc = self.next
    }

    fn csr(&mut self, address: CsrAddress, f: impl FnOnce(u64) -> u64) -> Result<u64, InvalidCsr> {
        match address {
            CsrAddress::SSTATUS => Ok(self.csr.status.access_mstatus(f)),
            CsrAddress::MSTATUS => Ok(self.csr.status.access_sstatus(f)),
            //CsrAddress::MISA => Ok(self.csr.misa.access(f)),
            CsrAddress::MEDELEG => Ok(self.csr.medeleg.access(f)),
            CsrAddress::SIE => Ok(self.csr.sie.access(f)),
            CsrAddress::MIE => Ok(self.csr.mie.access(f)),
            CsrAddress::STVEC => Ok(self.csr.stvec.access(f)),
            CsrAddress::MTVEC => Ok(self.csr.mtvec.access(f)),
            //CsrAddress::SCOUNTEREN => Ok(self.csr.scounteren.access(f)),
            //CsrAddress::MCOUNTEREN => Ok(self.csr.mcounteren.access(f)),
            CsrAddress::SSCRATCH => Ok(self.csr.sscratch.access(f)),
            CsrAddress::MSCRATCH => Ok(self.csr.mscratch.access(f)),
            CsrAddress::SEPC => Ok(self.csr.sepc.access(f)),
            CsrAddress::MEPC => Ok(self.csr.mepc.access(f)),
            CsrAddress::SCAUSE => Ok(self.csr.scause.access(f)),
            CsrAddress::MCAUSE => Ok(self.csr.mcause.access(f)),
            CsrAddress::STVAL => Ok(self.csr.stval.access(f)),
            CsrAddress::MTVAL => Ok(self.csr.mtval.access(f)),
            CsrAddress::SIP => Ok(self.csr.sip.access(f)),
            CsrAddress::MIP => Ok(self.csr.mip.access(f)),
            //CsrAddress::SATP => Ok(self.csr.satp.access(f)),
            CsrAddress::MVENDORID => Ok(self.csr.mvendorid.access(f)),
            CsrAddress::MARCHID => Ok(self.csr.marchid.access(f)),
            CsrAddress::MIMPID => Ok(self.csr.mimpid.access(f)),
            CsrAddress::MHARTID => Ok(self.csr.mhartid.access(f)),
            _ => Err(InvalidCsr),
        }
    }

    /// Sets up processor state to handle the given trap.
    fn trap(&mut self, cause: TrapCause, value: TrapValue) {
        if self.privilege == PrivilegeLevel::User {
            self.privilege = PrivilegeLevel::Supervisor;
        }

        if self.privilege == PrivilegeLevel::Supervisor {
            if self.csr.medeleg.is_delegated(cause) || self.csr.mideleg.is_delegated(cause) {
                return self.handle_trap_supervisor(cause, value);
            } else {
                self.privilege = PrivilegeLevel::Machine;
            }
        }

        self.handle_trap_machine(cause, value)
    }

    fn handle_trap_machine(&mut self, cause: TrapCause, value: TrapValue) {
        self.csr.mcause.set(cause);
        self.csr.mepc.set(if cause.is_interrupt() { self.next } else { self.pc });
        self.csr.mtval.set(value);

        if self.csr.mtvec.mode() == 0 {
            self.next = self.csr.mtvec.base();
        }
    }

    fn handle_trap_supervisor(&mut self, cause: TrapCause, value: TrapValue) {
        self.csr.scause.set(cause);
        self.csr.sepc.set(if cause.is_interrupt() { self.next } else { self.pc });
        self.csr.stval.set(value);

        if self.csr.stvec.mode() == 0 {
            self.next = self.csr.stvec.base();
        }
    }
}

impl TrapCause {
    const fn is_interrupt(self) -> bool {
        self as u64 & 1 << 63 != 0
    }

    const fn interrupt(code: u64) -> u64 {
        code | 1 << 63
    }

    const fn exception(code: u64) -> u64 {
        code
    }
}

impl TrapValue {
    /// Creates an empty trap value. Calling this function is equivalent to `TrapValue(0)`;
    /// however, it is preferred over `TrapValue(0)` to make the meaning clearer.
    pub const fn empty() -> TrapValue {
        TrapValue(0)
    }
}
