use std::num::{NonZeroU32, NonZeroU64};

mod instruction;
mod memory;

pub use memory::Memory;

/// An error that can be thrown on a memory access.
pub enum BusError {
    /// The address is invalid (but well-aligned).
    AccessFault,
    /// The address is misaligned.
    AddressMisaligned,
}

/// A bus facilitating accesses on the emulated physical bus.
pub trait Bus<A, V> {
    /// Loads the value located at the given `address`.
    fn load(&self, address: A) -> Result<V, BusError>;
    /// Stores the given `value` to the given `address`.
    fn store(&self, address: A, value: V) -> Result<(), BusError>;
}

/// A bus facilitating accesses to the emulated CSRs.
pub trait Csr {
    /// Attempts to access the CSR at the given `address` by setting the value
    /// of the CSR to the result of calling `f` with its current value.
    ///
    /// Returns the original value of the CSR on success.
    ///
    /// Returns `Err(CsrIllegal)` when the access is illegal for some reason,
    /// usually because the CSR does not exist or because the current privilege
    /// level does not have access.
    fn access(
        &mut self,
        address: CsrAddress,
        f: impl FnOnce(u64) -> u64,
    ) -> Result<u64, CsrIllegal>;
}

/// Returned by [Csr::access] to indicate an illegal CSR access.
pub struct CsrIllegal;

/// A wrapper for integers guaranteed to be less than 4096 that is used to represent
/// CSR addresses.
pub struct CsrAddress(u16);

/// Exceptions that can be encountered during the execution of an instruction
/// by a [BaseHart].
///
/// These mostly line up with the list of exceptions defined by the RISC-V
/// privlieged specification, though with some minor differences and omissions.
pub enum Exception {
    InstructionAddressMisaligned { address: Option<NonZeroU64> },
    InstructionAccessFault { address: Option<NonZeroU64> },
    IllegalInstruction { instruction: Option<NonZeroU32> },
    Breakpoint { address: Option<NonZeroU64> },
    LoadAddressMisaligned { address: Option<NonZeroU64> },
    LoadAccessFault { address: Option<NonZeroU64> },
    StoreAmoAddressMisaligned { address: Option<NonZeroU64> },
    StoreAmoAccessFault { address: Option<NonZeroU64> },
    EnvironmentCall,
}

/// A simple implementation of a processor that implements only machine mode.
pub struct BaseHart<B, C> {
    /// The system bus for reading and writing physical memory addresses.
    pub bus: B,
    /// The address of the currently-executing instruction, if one is being executed.
    pub pc: u64,
    /// The address of the next instruction.
    pub next: u64,
    /// The general-purpose registers x0 through x31.
    pub gpr: [u64; 32],
    /// The state of all control and status registers.
    pub csr: C,
    /// The result that will be returned after the current instruction finishes.
    result: Result<(), Exception>,
}

impl<B, C> BaseHart<B, C> {
    /// Creates a new `Hart` that connects to `bus` for memory accesses.
    pub fn new(bus: B, csr: C) -> BaseHart<B, C> {
        BaseHart {
            bus,
            pc: 0,
            next: 0,
            gpr: [0; 32],
            csr,
            result: Ok(()),
        }
    }

    /// Executes one instruction at the given address, returning the offset
    /// that should be applied to the PC to execute the next instruction.
    ///
    /// The address may have any alignment; only PC offsets are checked for
    /// alignment.
    pub fn execute(&mut self) -> Result<(), Exception>
    where
        B: Bus<u64, u64> + Bus<u64, u32> + Bus<u64, u16> + Bus<u64, u8>,
        C: Csr,
    {
        // Fetch
        let raw: u32 = match self.bus.load(self.pc) {
            Ok(raw) => raw,
            Err(BusError::AccessFault) => {
                return Err(Exception::InstructionAccessFault {
                    address: NonZeroU64::new(self.pc),
                })
            }
            Err(BusError::AddressMisaligned) => {
                return Err(Exception::InstructionAddressMisaligned {
                    address: NonZeroU64::new(self.pc),
                })
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
            _ => |hart: &mut BaseHart<B, C>, raw| {
                hart.raise(Exception::IllegalInstruction {
                    instruction: NonZeroU32::new(raw),
                })
            },
        };

        instruction(self, raw);

        self.pc = self.next;

        let mut result = Ok(());
        std::mem::swap(&mut self.result, &mut result);
        result
    }

    /// Sets up state for the given exception to be raised after execution is finished.
    fn raise(&mut self, exception: Exception) {
        self.result = Err(exception);
    }
}

impl CsrAddress {
    pub const fn new(address: u16) -> Option<CsrAddress> {
        if address < 4096 {
            Some(CsrAddress(address))
        } else {
            None
        }
    }

    pub const unsafe fn new_unchecked(address: u16) -> CsrAddress {
        CsrAddress(address)
    }
}
