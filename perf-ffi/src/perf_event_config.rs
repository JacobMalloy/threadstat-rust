use crate::sys;
use sys::perf_event_attr;

#[derive(Clone, Copy, Debug)]
pub struct PerfConfig(pub(crate) sys::perf_event_attr);

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum HardwareEvent {
    CpuCycles = sys::perf_hw_id_PERF_COUNT_HW_CPU_CYCLES,
    Instructions = sys::perf_hw_id_PERF_COUNT_HW_INSTRUCTIONS,
    CacheReferences = sys::perf_hw_id_PERF_COUNT_HW_CACHE_REFERENCES,
    CacheMisses = sys::perf_hw_id_PERF_COUNT_HW_CACHE_MISSES,
    BranchInstructions = sys::perf_hw_id_PERF_COUNT_HW_BRANCH_INSTRUCTIONS,
    BranchMisses = sys::perf_hw_id_PERF_COUNT_HW_BRANCH_MISSES,
    BusCycles = sys::perf_hw_id_PERF_COUNT_HW_BUS_CYCLES,
    StalledCyclesFrontend = sys::perf_hw_id_PERF_COUNT_HW_STALLED_CYCLES_FRONTEND,
    StalledCyclesBackend = sys::perf_hw_id_PERF_COUNT_HW_STALLED_CYCLES_BACKEND,
    RefCpuCycles = sys::perf_hw_id_PERF_COUNT_HW_REF_CPU_CYCLES,
}

impl HardwareEvent {
    fn get_value(&self) -> u32 {
        (*self) as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum CacheId {
    L1D = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_L1D,
    L1I = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_L1I,
    LL = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_LL,
    DTLB = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_DTLB,
    ITLB = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_ITLB,
    BPU = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_BPU,
    Node = sys::perf_hw_cache_id_PERF_COUNT_HW_CACHE_NODE,
}

impl CacheId {
    fn get_value(&self) -> u32 {
        (*self) as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum CacheOperation {
    Read = sys::perf_hw_cache_op_id_PERF_COUNT_HW_CACHE_OP_READ,
    Write = sys::perf_hw_cache_op_id_PERF_COUNT_HW_CACHE_OP_WRITE,
    Prefetch = sys::perf_hw_cache_op_id_PERF_COUNT_HW_CACHE_OP_PREFETCH,
}

impl CacheOperation {
    fn get_value(&self) -> u32 {
        (*self) as u32
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum CacheResult {
    Access = sys::perf_hw_cache_op_result_id_PERF_COUNT_HW_CACHE_RESULT_ACCESS,
    Miss = sys::perf_hw_cache_op_result_id_PERF_COUNT_HW_CACHE_RESULT_MISS,
}

impl CacheResult {
    fn get_value(&self) -> u32 {
        (*self) as u32
    }
}

impl PerfConfig {
    unsafe fn default_perf_event_attr() -> sys::perf_event_attr {
        let mut rv: perf_event_attr = unsafe { core::mem::zeroed() };
        rv.size = size_of::<sys::perf_event_attr>() as u32;
        rv
    }

    pub fn set_exlude_hv(mut self, value: bool) -> Self {
        self.0.set_exclude_hv(if value { 1 } else { 0 });
        self
    }

    pub fn hardware_event(event: HardwareEvent) -> Self {
        let mut attr = unsafe { Self::default_perf_event_attr() };
        attr.type_ = sys::perf_type_id_PERF_TYPE_HARDWARE;
        attr.config = event.get_value() as u64;
        PerfConfig(attr).set_exlude_hv(true)
    }

    pub fn hardware_cache_event(cache: CacheId, op: CacheOperation, result: CacheResult) -> Self {
        let mut attr = unsafe { Self::default_perf_event_attr() };
        attr.type_ = sys::perf_type_id_PERF_TYPE_HW_CACHE;
        attr.config =
            (cache.get_value() | (op.get_value() << 8) | (result.get_value() << 16)) as u64;
        PerfConfig(attr).set_exlude_hv(true)
    }
}


