use crate::amf0::define::Amf0ValueType;
use bytes::BytesMut;

pub struct SetPeerBandwidthProperties {
    window_size: u32,
    limit_type: u8,
}

impl SetPeerBandwidthProperties {
    pub fn new(window_size: u32, limit_type: u8) -> Self {
        Self {
            window_size: window_size,
            limit_type: limit_type,
        }
    }
}

pub enum MessageTypes {
    Amf0Command {
        command_name: Amf0ValueType,
        transaction_id: Amf0ValueType,
        command_object: Amf0ValueType,
        others: Vec<Amf0ValueType>,
    },
    SetChunkSize {
        chunk_size: u32,
    },
    AbortMessage {
        chunk_stream_id: u32,
    },
    Acknowledgement {
        sequence_number: u32,
    },
    WindowAcknowledgementSize {
        size: u32,
    },
    SetPeerBandwidth {
        properties: SetPeerBandwidthProperties,
    },
    AudioData {
        data: BytesMut,
    },
    VideoData {
        data: BytesMut,
    },
}

pub mod msg_type_id {
    pub const AUDIO: u8 = 8;
    pub const VIDEO: u8 = 9;

    pub const SET_CHUNK_SIZE: u8 = 1;
    pub const ABORT: u8 = 2;
    pub const ACKNOWLEDGEMENT: u8 = 3;
    pub const USER_CONTROL_EVENT: u8 = 4;
    pub const WIN_ACKNOWLEDGEMENT_SIZE: u8 = 5;
    pub const SET_PEER_BANDWIDTH: u8 = 6;

    pub const COMMAND_AMF3: u8 = 17;
    pub const COMMAND_AMF0: u8 = 20;

    pub const DATA_AMF3: u8 = 15;
    pub const DATA_AMF0: u8 = 18;

    pub const SHARED_OBJ_AMF3: u8 = 16;
    pub const SHARED_OBJ_AMF0: u8 = 19;

    pub const AGGREGATE: u8 = 22;
}