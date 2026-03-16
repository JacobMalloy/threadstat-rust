use non_empty::{MaybeNonEmpty, NonEmpty};
use perf_ffi::PfmError;
use poll::{PollAction, Poller};
use signals::{Signal, SignalFD};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use string_intern::Intern;

use clap::Parser;

mod threadstat_message;
use threadstat_message::{MessageFlag, ThreadstatMessage, ThreadstatMQReader, MQ_NAME};

static READ_ID: AtomicU64 = AtomicU64::new(0);

#[derive(clap::Parser)]
struct Args {
    /// Required input (one or more words)
    #[arg(required = true, num_args = 1..)]
    events: Vec<String>,

    #[arg(short, long, default_value = "./")]
    output_folder: PathBuf,

}

/// Parse "event1,[event2,event3],event4" into Vec<Vec<PerfConfig<Intern>>>.
/// Top-level commas separate groups; [..] brackets group multiple events together.
fn parse_event_groups(s: &str) -> Result<Vec<NonEmpty<perf_ffi::PerfConfig<Intern>>>, PfmError> {
    let mut groups = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;

    loop {
        while i < b.len() && (b[i] == b',' || b[i].is_ascii_whitespace()) {
            i += 1;
        }

        if i >= b.len() {
            break;
        }

        if b[i] == b'[' {
            i += 1; // consume '['
            let start = i;
            while i < b.len() && b[i] != b']' {
                i += 1;
            }
            let inner = &s[start..i];
            if i < b.len() {
                i += 1; // consume ']'
            }
            let group: Result<MaybeNonEmpty<_>, PfmError> = inner
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .map(|e| {
                    let interned = Intern::new(e);
                    perf_ffi::PerfConfig::from_pfm_string(interned, interned)
                })
                .collect();
            if let Some(v) = group?.into_option() {
                groups.push(v);
            }
        } else {
            let start = i;
            while i < b.len() && b[i] != b',' && b[i] != b'[' {
                i += 1;
            }
            let name = s[start..i].trim();
            if !name.is_empty() {
                let interned = Intern::new(name);
                groups.push(NonEmpty::new_single(
                    perf_ffi::PerfConfig::from_pfm_string(interned, interned)?,
                ));
            }
        }
    }

    Ok(groups)
}

struct CsvWriters {
    event: BufWriter<File>,
    read: BufWriter<File>,
    desc: BufWriter<File>,
}

impl CsvWriters {
    fn open(folder: &PathBuf) -> std::io::Result<Self> {
        let mut event = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(folder.join("threadstat-event.csv"))?,
        );
        let mut read = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(folder.join("threadstat-read.csv"))?,
        );
        let mut desc = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(folder.join("threadstat-event-description.csv"))?,
        );
        writeln!(event, "read_id,count,event_id")?;
        writeln!(read, "read_id,timestamp,time_running,time_enabled")?;
        writeln!(desc, "event_id,name,pid")?;
        Ok(Self { event, read, desc })
    }
}

struct State {
    event_configs: Vec<NonEmpty<perf_ffi::PerfConfig<Intern>>>,
    groups: HashMap<i32, Vec<perf_ffi::PerfEventGroup<Intern>>>,
    reader: perf_ffi::PerfGroupReader,
    csv: CsvWriters,
}

impl State {
    fn open_tid(&mut self, tid: i32) {
        if self.groups.contains_key(&tid) {
            return;
        }
        let tid_groups: Vec<_> = self
            .event_configs
            .iter()
            .filter_map(|config| {
                match perf_ffi::PerfEventGroup::new(config.iter(), tid) {
                    Ok(g) => Some(g),
                    Err(e) => {
                        eprintln!("failed to open events for tid {tid}: {e}");
                        None
                    }
                }
            })
            .collect();

        // Write event descriptions
        for group in &tid_groups {
            for (name, id) in group.name_and_ids().filter_map(|r| r.ok()) {
                if let Err(e) = writeln!(self.csv.desc, "{id},{name},{tid}") {
                    eprintln!("desc csv write error: {e}");
                }
            }
        }

        self.groups.insert(tid, tid_groups);
    }

    fn flush_tid(&mut self, tid: i32) {
        let Some(groups) = self.groups.get(&tid) else {
            return;
        };
        for group in groups {
            let read_id = READ_ID.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            match self.reader.read_group(group) {
                Ok((group_info, events)) => {
                    if let Err(e) = writeln!(
                        self.csv.read,
                        "{read_id},{timestamp},{},{}",
                        group_info.time_running, group_info.time_enabled
                    ) {
                        eprintln!("read csv write error: {e}");
                    }
                    for e in events {
                        if let Err(err) =
                            writeln!(self.csv.event, "{read_id},{},{}", e.count, e.id)
                        {
                            eprintln!("event csv write error: {err}");
                        }
                    }
                }
                Err(e) => eprintln!("read error for tid {tid}: {e}"),
            }
        }
    }

    fn close_tid(&mut self, tid: i32) {
        self.flush_tid(tid);
        self.groups.remove(&tid);
    }

    fn flush_all(&mut self) {
        let tids: Vec<i32> = self.groups.keys().copied().collect();
        for tid in tids {
            self.flush_tid(tid);
        }
    }
}

fn main() {
    let mq = ThreadstatMQReader::new(MQ_NAME).expect("failed to open mqueue");
    println!("Opened Message Queue");
    let Args {
        events,
        output_folder,
    } = Args::parse();
    println!("Parsed Args");

    let event_string = events.join(",");
    let event_configs = parse_event_groups(&event_string).expect("Failed to parse events");
    let csv = CsvWriters::open(&output_folder).expect("failed to open csv files");

    let mut state = State {
        event_configs,
        groups: HashMap::new(),
        reader: perf_ffi::PerfGroupReader::default(),
        csv,
    };

    Signal::block([Signal::SIGINT]).expect("failed to block SIGINT");
    let signal_fd = SignalFD::new([Signal::SIGINT]).expect("failed to create signalfd");
    println!("Setup Signal Handling");

    {
        let mut poller = Poller::new();
        poller.register(&signal_fd, || {
            signal_fd.read().expect("failed to read signalfd");
            Ok(PollAction::Stop)
        });
        poller.register(&mq, || {
            match mq.read() {
                Ok(raw) => match ThreadstatMessage::try_from(raw) {
                    Ok(msg) => match msg.flag {
                        MessageFlag::AddProcess => state.open_tid(msg.tid),
                        MessageFlag::RemoveProcess => state.close_tid(msg.tid),
                    },
                    Err(e) => eprintln!("mqueue bad message: {e}"),
                },
                Err(e) => eprintln!("mqueue read error: {e}"),
            }
            Ok(PollAction::Continue)
        });
        poller.run().expect("poll error");
    } // poller and closures dropped, releasing the mutable borrow of state
    println!("Finished the polling loop");
    state.flush_all();
}
