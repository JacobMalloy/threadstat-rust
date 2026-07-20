use threadstat_message::{ThreadstatMQReader, ThreadstatMessage,MessageFlag};
use poll::PollAction;
use crate::State;

pub fn queue_handler(mq: &mut ThreadstatMQReader, state: &mut State) -> Result<PollAction, std::io::Error> {
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
}
