use std::{fs, io, path::Path};

use irv::{BaseHart, Bus, Exception, Memory};

const TEST_BUS_BASE: u64 = 0x80000000;
const MAX_INSTRET: usize = 1000;

struct TestBus(Memory<Vec<u64>>);

macro_rules! impl_test_bus {
    ($($ty:ident)*) => {
        $(
            impl Bus<u64, $ty> for TestBus {
                fn load(&self, address: u64) -> Result<$ty, irv::BusError> {
                    self.0.load(address.wrapping_sub(TEST_BUS_BASE))
                }

                fn store(&self, address: u64, value: $ty) -> Result<(), irv::BusError> {
                    self.0.store(address.wrapping_sub(TEST_BUS_BASE), value)
                }
            }
        )*
    };
}

impl_test_bus! { u8 u16 u32 u64 }

#[test]
fn test_riscv_tests() -> Result<(), io::Error> {
    // load each test file
    for entry in fs::read_dir(Path::new("../riscv-tests/isa"))? {
        let entry = entry?;

        if let Ok(file_name) = entry.file_name().into_string() {
            if file_name.starts_with("rv64ui-irv-") && !file_name.ends_with(".dump") {
                println!("Testing {file_name}...");
                test_riscv_test(fs::read(entry.path())?)
            }
        }
    }

    Ok(())
}

fn test_riscv_test(elf_data: Vec<u8>) {
    let bus = TestBus(Memory::new(vec![0u64; 8000]));

    irv_loader::load(&bus, &elf_data).expect("Failed to parse or load ELF");

    let mut hart = BaseHart::new(bus, ());

    hart.pc = TEST_BUS_BASE;
    hart.next = TEST_BUS_BASE;

    for instret in 0.. {
        if instret > MAX_INSTRET {
            panic!("Test took too many ({instret}) instructions! Is it in an infinite loop, or does MAX_INSTRET need to be increased?")
        }

        match hart.execute() {
            Ok(()) => (),
            Err(Exception::EnvironmentCall) => {
                if hart.gpr[17] != 93 || hart.gpr[10] != 0 {
                    // fail
                    println!("Test reports as failing from within RISC-V");
                    dbg!(hart.pc, hart.gpr);
                    panic!();
                } else {
                    // pass
                    break;
                }
            }
            Err(e) => panic!("Unexpected exception thrown during execution: {e:#?}"),
        }
    }
}
