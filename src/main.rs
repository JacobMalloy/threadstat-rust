use non_empty::{MaybeNonEmpty, NonEmpty};
use perf_ffi::PfmError;
use std::path::PathBuf;
use std::{thread::sleep, time::Duration};
use string_intern::Intern;

use clap::Parser;

#[derive(clap::Parser)]
struct Args {
    /// Required input (one or more words)
    #[arg(required = true, num_args = 1..)]
    events: Vec<String>,

    #[arg(short, long, default_value = "./")]
    output_folder: PathBuf,

    #[arg(short, long, required = true)]
    pid: i64,
}

/// Parse "event1,[event2,event3],event4" into Vec<Vec<(PerfConfig, String)>>.
/// Top-level commas separate groups; [..] brackets group multiple events together.
fn parse_event_groups(s: &str) -> Result<Vec<NonEmpty<(perf_ffi::PerfConfig, Intern)>>, PfmError> {
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
                    Ok((perf_ffi::PerfConfig::from_pfm_string(interned)?, interned))
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
                groups.push(NonEmpty::new_single((
                    perf_ffi::PerfConfig::from_pfm_string(interned)?,
                    interned,
                )));
            }
        }
    }

    Ok(groups)
}

fn main() {
    let Args {
        events,
        output_folder,
        pid,
    } = Args::parse();

    let event_string = events.join(",");
    let event_groups = parse_event_groups(&event_string).expect("Failed to parse events");

    let tmp: Vec<perf_ffi::PerfEventGroup<Intern>> = event_groups
        .into_iter()
        .map(|group| {
            let (configs, names): (Vec<_>, Vec<_>) = group.into_iter().unzip();
            perf_ffi::PerfEventGroup::new(configs.iter().zip(names), pid as i32).unwrap()
        })
        .collect();

    let mut reader = perf_ffi::PerfGroupReader::default();
    sleep(Duration::from_secs_f32(30.0));

    for group in tmp.iter() {
        for x in reader.read_group(group).unwrap() {
            println!("{:?}", x);
        }
    }
}
