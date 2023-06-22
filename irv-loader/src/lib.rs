use irv_traits::{Bus, BusError};

/// An error returned by [`load`].
#[derive(Debug)]
pub enum Error {
    /// An error encountered while writing to the bus.
    Bus(BusError),
    /// An error encountered while parsing the ELF file.
    Goblin(goblin::error::Error),
}

impl From<goblin::error::Error> for Error {
    fn from(e: goblin::error::Error) -> Self {
        Error::Goblin(e)
    }
}

impl From<BusError> for Error {
    fn from(e: BusError) -> Self {
        Error::Bus(e)
    }
}

/// Attempts to parse `elf_data` as an ELF file and store the contents of all
/// of its loadable segments into appropriate addresses on the `bus`.
pub fn load<B>(bus: &B, elf_data: &[u8]) -> Result<(), Error>
where
    B: Bus<u64, u8>,
{
    let elf = goblin::elf::Elf::parse(elf_data)?;

    for phdr in elf.program_headers {
        let data = &elf_data[phdr.file_range()];
        let vm_range = phdr.vm_range();

        let mut vm_addr = phdr.vm_range().start as u64;
        for byte in data {
            if vm_addr == vm_range.end as u64 {
                break;
            }

            bus.store(vm_addr, *byte)?;

            vm_addr += 1;
        }
    }

    Ok(())
}
