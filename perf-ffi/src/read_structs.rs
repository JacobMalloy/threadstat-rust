use zerocopy::{FromBytes, IntoBytes, KnownLayout, Immutable};

#[derive(Clone,Copy , FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct PerfGroupReadHeader{
    pub nr:u64,
    pub time_enabled:u64,
    pub time_running:u64,
}

#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct PerfGroupReadEntry{
    pub value:u64,
    pub id:u64,
}

