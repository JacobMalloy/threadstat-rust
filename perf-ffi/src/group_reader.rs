use crate::perf_event::PerfEventGroup;
use crate::read_structs;
use core::alloc::{Layout, LayoutError};
use core::ptr::NonNull;
use core::slice;
use std::alloc;
use std::alloc::{alloc, dealloc, realloc};

struct PerfGroupReaderInner {
    data: NonNull<u8>,
    layout: Layout,
}

pub struct PerfGroupReader(Option<PerfGroupReaderInner>);

#[derive(Clone, Copy, Debug)]
pub struct GroupInfo {
    pub time_enabled: u64,
    pub time_running: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct EventInfo {
    pub id: u64,
    pub count: u64,
}

const fn necessary_buffer_size(count: usize) -> usize {
    size_of::<read_structs::PerfGroupReadHeader>()
        + count * size_of::<read_structs::PerfGroupReadEntry>()
}

impl PerfGroupReader {
    const fn get_layout_from_count(count: usize) -> Result<Layout, LayoutError> {
        alloc::Layout::from_size_align(
            necessary_buffer_size(count),
            align_of::<read_structs::PerfGroupReadHeader>(),
        )
    }

    fn ensure_sized(&'_ mut self, count: usize) -> &'_ mut [u8] {
        let min_size = necessary_buffer_size(count);

        let tmp = match self.0.take() {
            Some(PerfGroupReaderInner { data, layout }) => {
                if min_size > layout.size() {
                    let l = Self::get_layout_from_count(count).expect("Invalid Layout");
                    let p = unsafe { realloc(data.as_ptr(), layout, min_size) };
                    let nn = NonNull::new(p).expect("Realloc returned NULL");
                    PerfGroupReaderInner {
                        data: nn,
                        layout: l,
                    }
                } else {
                    PerfGroupReaderInner { data, layout }
                }
            }
            None => {
                let l = Self::get_layout_from_count(count).expect("Invalid Layout");
                let p = unsafe { alloc(l) };
                let nn = NonNull::new(p).expect("Allocation returned NULL");
                PerfGroupReaderInner {
                    data: nn,
                    layout: l,
                }
            }
        };
        let ptr = tmp.data;
        self.0 = Some(tmp);
        unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), min_size) }
    }

    pub fn new() -> Self {
        PerfGroupReader(None)
    }

    pub fn read_group<'a, T>(
        &'a mut self,
        group: &PerfEventGroup<T>,
    ) -> Result<(GroupInfo, impl Iterator<Item = EventInfo> + 'a), crate::error::Error<'a>> {
        let buf = self.ensure_sized(group.len());
        let (header, events) = group.read(buf)?;
        let group_info = GroupInfo {
            time_enabled: header.time_enabled,
            time_running: header.time_running,
        };
        let event_iter = events.iter().map(|e| EventInfo { id: e.id, count: e.value });
        Ok((group_info, event_iter))
    }
}

impl Default for PerfGroupReader{
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PerfGroupReader {
    fn drop(&mut self) {
        if let Some(ref v) = self.0 {
            unsafe {
                dealloc(v.data.as_ptr(), v.layout);
            }
        }
    }
}
