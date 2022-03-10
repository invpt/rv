pub enum BusError {
    AccessFault,
    AddressMisaligned,
}

pub trait Bus<A, V> {
    fn load(&self, address: A) -> Result<V, BusError>;
    fn store(&self, address: A, value: V) -> Result<(), BusError>;
}