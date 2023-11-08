use once_cell::sync::Lazy;
use std::fs::File;
use std::io::Write;

use super::pipe_pool::Message;

pub fn trace_outgoing_msg(msg: &Message) {
    static TRACE_OUTGOING: Lazy<Option<File>> = Lazy::new(|| {
        if let Ok(fname) = std::env::var("SOSISTAB_TRACE_OUTGOING") {
            let mut file =
                File::create(&fname).expect("cannot create file for SOSISTAB_TRACE_OUTGOING");
            writeln!(file, "kind,stream_id,seqno,payload_len").unwrap();
            Some(File::create(&fname).expect("cannot create file for SOSISTAB_TRACE_OUTGOING"))
        } else {
            None
        }
    });

    if let Some(mut inner) = TRACE_OUTGOING.as_ref() {
        if let Message::Rel {
            kind,
            stream_id,
            seqno,
            payload,
        } = msg
        {
            let _ = writeln!(inner, "{:?},{stream_id},{seqno},{}", kind, payload.len());
        }
    }
}

pub fn trace_incoming_msg(msg: &Message) {
    static TRACE_INCOMING: Lazy<Option<File>> = Lazy::new(|| {
        if let Ok(fname) = std::env::var("SOSISTAB_TRACE_INCOMING") {
            let mut file =
                File::create(&fname).expect("cannot create file for SOSISTAB_TRACE_INCOMING");
            writeln!(file, "kind,stream_id,seqno,payload_len").unwrap();
            Some(File::create(&fname).expect("cannot create file for SOSISTAB_TRACE_INCOMING"))
        } else {
            None
        }
    });

    if let Some(mut inner) = TRACE_INCOMING.as_ref() {
        if let Message::Rel {
            kind,
            stream_id,
            seqno,
            payload,
        } = msg
        {
            let _ = writeln!(inner, "{:?},{stream_id},{seqno},{}", kind, payload.len());
        }
    }
}