use core::{
    mem::{align_of, size_of},
    ops::{Deref, DerefMut},
    pin::Pin,
};

use crate::{Bus, BusError};

/// An efficient implementation of main memory.
pub struct Memory<T> {
    data: Pin<T>,
    data_ptr: *mut u8,
    data_len: usize,
}

impl_bus! {
    u64 u8,
    u64 u16,
    u64 u32,
    u64 u64,
}

impl<T> Memory<T> {
    /// Converts a slice to be used as memory for a hart.
    pub fn new(data: T) -> Memory<T>
    where
        T: Deref<Target = [u64]> + DerefMut + Unpin,
    {
        let mut data = Pin::new(data);

        let data_ref = &mut *data;

        let data_ptr = data_ref.as_mut_ptr() as *mut u8;
        let data_len = data_ref.len() * size_of::<u64>();

        Memory {
            data,
            data_ptr,
            data_len,
        }
    }

    /// Gets the size of memory in bytes.
    pub const fn size(&self) -> usize {
        self.data_len
    }

    /// Gets back the original data used as backing for this memory.
    pub fn into_data(self) -> T
    where
        T: Deref<Target = [u64]> + Unpin,
    {
        Pin::into_inner(self.data)
    }

    #[inline]
    fn calculate_destination<V>(&self, address: usize) -> Result<*mut V, BusError> {
        let upper = address.wrapping_add(size_of::<V>());

        if address < upper && upper <= self.size() {
            // SAFETY: We know that `address` < `self.size()`. Also, we know that
            // `self.size()` < `isize::MAX`, since this is a requirement on the
            // length of slices. Finally, we know it lies within the same allocated
            // object and does not wrap around per the condition of the enclosing if statement.
            let ptr = unsafe { self.data_ptr.add(address) } as *mut V;

            if ptr as usize % align_of::<V>() == 0 {
                Ok(ptr)
            } else {
                Err(BusError::AddressMisaligned)
            }
        } else {
            Err(BusError::AccessFault)
        }
    }
}

macro_rules! impl_bus {
    ($($addr:ident $val:ident,)*) => {
        $(impl<T> Bus<$addr, $val> for Memory<T> {
            fn load(&self, address: $addr) -> Result<$val, BusError> {
                if address as usize as u64 == address {
                    let ptr = self.calculate_destination(address as usize)?;

                    // SAFETY: check_address returns a pointer that is guaranteed to be valid.
                    Ok(unsafe { *ptr })
                } else {
                    Err(BusError::AccessFault)
                }
            }

            fn store(&self, address: $addr, value: $val) -> Result<(), BusError> {
                if address as usize as u64 == address {
                    let ptr = self.calculate_destination(address as usize)?;

                    // SAFETY: check_address returns a pointer that is guaranteed to be valid.
                    Ok(unsafe { *ptr = value })
                } else {
                    Err(BusError::AccessFault)
                }
            }
        })*
    };
}

use impl_bus;
