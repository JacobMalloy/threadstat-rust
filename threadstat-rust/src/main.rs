use std::time::Duration;

use poll::{PollAction, Poller};
use signals::{Signal, SignalFD};
use threadstat_shared::{BaseArgs, State, handlers, parse_perf};
use timerfd::{Clock, TimerFD};

use clap::Parser;

use threadstat_message::{MQ_NAME, ThreadstatMQReader};

#[derive(clap::Parser)]
struct Args {
    /// Required input (one or more words)
    #[command(flatten)]
    base: BaseArgs,

    /// Flush counters this many times per second. 0 flushes only at exit.
    #[arg(short, long, default_value_t = 0)]
    frequency: u32,
}

fn main() {
    // Parse before opening anything, so --help and usage errors do not have to get past the
    // mqueue first.
    let Args {
        base: BaseArgs {
            events,
            output_folder,
        },
        frequency,
    } = Args::parse();
    println!("Parsed Args");

    let mut mq = ThreadstatMQReader::new(MQ_NAME).expect("failed to open mqueue");
    println!("Opened Message Queue");

    let event_string = events.join(",");
    let event_configs =
        parse_perf::parse_event_groups(&event_string).expect("Failed to parse events");

    let mut state = State::new(event_configs, &output_folder);

    let close_signals = [Signal::SIGINT];
    Signal::block(close_signals).expect("failed to block SIGINT");
    let mut signal_fd = SignalFD::new(close_signals).expect("failed to create signalfd");

    // Monotonic so the flush cadence survives a wall-clock step.
    let mut flush_timer = (frequency > 0).then(|| {
        let mut timer = TimerFD::new(Clock::Monotonic).expect("failed to create timerfd");
        let period = Duration::from_secs_f64(1.0 / f64::from(frequency));
        timer
            .arm_periodic(period)
            .expect("failed to arm the flush timer");
        println!("Flushing every {period:?}");
        timer
    });

    println!("Setup Signal Handling");

    let mut poller = Poller::new();
    poller.register_mut(&mut signal_fd, |s, _state: &mut State| {
        s.read().expect("failed to read signalfd");
        Ok(PollAction::Stop)
    });
    poller.register_mut(&mut mq, handlers::queue_handler);
    if let Some(timer) = &mut flush_timer {
        poller.register_mut(timer, |timer, state: &mut State| {
            // The timerfd reports every expiration since the last read, so anything above one
            // means a flush ran longer than the sampling period and ticks were missed.
            let expirations = timer.read()?;
            if expirations != 1 {
                eprintln!(
                    "flush fell behind: {expirations} periods elapsed since the previous flush"
                );
            }
            state.flush_all();
            Ok(PollAction::Continue)
        });
    }

    poller.run(&mut state).expect("poll error");

    println!("Finished the polling loop");
    state.flush_all();
}
