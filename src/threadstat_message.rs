mod raw {
    #![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code)]
    include!(concat!(env!("OUT_DIR"), "/shared_def_bindings.rs"));
}

use raw::message;

pub type ThreadstatMQReader<'a> = mqueue::MQueueReader<'a, message>;

pub const MQ_NAME: &std::ffi::CStr =
    match std::ffi::CStr::from_bytes_with_nul(raw::THREADSTAT_MQ) {
        Ok(s) => s,
        Err(_) => panic!("invalid THREADSTAT_MQ constant"),
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageFlag {
    AddProcess,
    RemoveProcess,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadstatMessage {
    pub tid: i32,
    pub flag: MessageFlag,
}

#[derive(Debug)]
pub struct UnknownFlag(pub i32);

impl std::fmt::Display for UnknownFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown message flag: {}", self.0)
    }
}

impl std::error::Error for UnknownFlag {}

impl TryFrom<message> for ThreadstatMessage {
    type Error = UnknownFlag;

    fn try_from(msg: message) -> Result<Self, Self::Error> {
        let flag = match msg.flags {
            x if x == raw::MESSAGE_FLAG_ADD_PROCESS as i32 => MessageFlag::AddProcess,
            x if x == raw::MESSAGE_FLAG_REMOVE_PROCESS as i32 => MessageFlag::RemoveProcess,
            x => return Err(UnknownFlag(x)),
        };
        Ok(ThreadstatMessage { tid: msg.tid, flag })
    }
}
