use std::fs::{File, OpenOptions};
use std::io;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct CsvWriter {
    event: BufWriter<File>,
    read: BufWriter<File>,
    desc: BufWriter<File>,
}

impl CsvWriter {
    pub fn open(folder: &Path) -> std::io::Result<Self> {
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

    pub fn write_event(&mut self, read_id: u64, count: u64, id: u64) -> Result<(), io::Error> {
        writeln!(self.event, "{read_id},{count},{id}")
    }

    pub fn write_read(
        &mut self,
        read_id: u64,
        timestamp: u128,
        time_running: u64,
        time_enabled: u64,
    ) -> Result<(), io::Error> {
        writeln!(
            self.read,
            "{read_id},{timestamp},{time_running},{time_enabled}"
        )
    }

    pub fn write_desc(&mut self, id: u64, name: &str, tid: i32) -> Result<(), io::Error> {
        writeln!(self.desc, "{id},{name},{tid}")
    }
}
