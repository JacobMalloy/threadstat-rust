use crate::CsvWriter;
use non_empty::NonEmpty;
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use string_intern::InternC;

pub struct State {
    event_configs: Box<[NonEmpty<perf_ffi::PerfConfig<InternC>>]>,
    groups: HashMap<i32, Box<[perf_ffi::PerfStatGroup<InternC>]>>,
    reader: perf_ffi::PerfGroupReader,
    csv: CsvWriter,
    read_id: u64,
}

impl State {
    pub fn open_tid(&mut self, tid: i32) {
        if self.groups.contains_key(&tid) {
            return;
        }
        let tid_groups: Box<[_]> = self
            .event_configs
            .iter()
            .filter_map(
                |config| match perf_ffi::PerfStatGroup::new(config.iter(), tid) {
                    Ok(g) => Some(g),
                    Err(e) => {
                        eprintln!("failed to open events for tid {tid}: {e}");
                        None
                    }
                },
            )
            .collect();

        // Write event descriptions
        for group in &tid_groups {
            for (name, id) in group.name_and_ids().filter_map(Result::ok) {
                if let Err(e) = self.csv.write_desc(id, name, tid) {
                    eprintln!("desc csv write error: {e}");
                }
            }
        }

        self.groups.insert(tid, tid_groups);
    }

    fn flush_groups(
        groups: &[perf_ffi::PerfStatGroup<InternC>],
        tid: i32,
        reader: &mut perf_ffi::PerfGroupReader,
        csv: &mut CsvWriter,
        read_id_arg: &mut u64,
    ) {
        for group in groups {
            let read_id = *read_id_arg;
            *read_id_arg += 1;
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            match reader.read_group(group) {
                Ok((group_info, events)) => {
                    if let Err(e) = csv.write_read(
                        read_id,
                        timestamp,
                        group_info.time_running,
                        group_info.time_enabled,
                    ) {
                        eprintln!("read csv write error: {e}");
                    }
                    for e in events {
                        if let Err(err) = csv.write_event(read_id, e.count, e.id) {
                            eprintln!("event csv write error: {err}");
                        }
                    }
                }
                Err(e) => eprintln!("read error for tid {tid}: {e}"),
            }
        }
    }

    pub fn close_tid(&mut self, tid: i32) {
        if let Some(groups) = self.groups.remove(&tid) {
            Self::flush_groups(
                &groups,
                tid,
                &mut self.reader,
                &mut self.csv,
                &mut self.read_id,
            );
        }
    }

    pub fn flush_all(&mut self) {
        for (&tid, groups) in &self.groups {
            Self::flush_groups(
                groups,
                tid,
                &mut self.reader,
                &mut self.csv,
                &mut self.read_id,
            );
        }
    }

    pub fn new(
        event_configs: Box<[NonEmpty<perf_ffi::PerfConfig<InternC>>]>,
        output_folder: &Path,
    ) -> Self {
        Self {
            event_configs,
            groups: HashMap::new(),
            reader: perf_ffi::PerfGroupReader::default(),
            csv: CsvWriter::open(output_folder).expect("Failed To Open CSV's in output Folder"),
            read_id: 0,
        }
    }
}
