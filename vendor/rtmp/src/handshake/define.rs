pub enum SchemaVersion {
    Schema0,
    Schema1,
    Unknown,
}
#[derive(PartialEq)]
pub enum ClientHandshakeState {
    WriteC0C1,
    ReadS0S1S2,
    WriteC2,
    Finish,
}
#[derive(Copy, Clone)]
pub enum ServerHandshakeState {
    ReadC0C1,
    WriteS0S1S2,
    ReadC2,
    Finish,
}

pub const RTMP_VERSION: usize = 3;
pub const RTMP_HANDSHAKE_SIZE: usize = 1536;

pub const RTMP_SERVER_VERSION: [u8; 4] = [0x0D, 0x0E, 0x0A, 0x0D];
pub const RTMP_CLIENT_VERSION: [u8; 4] = [0x0C, 0x00, 0x0D, 0x0E];

pub const RTMP_DIGEST_LENGTH: usize = 32;
pub const RTMP_SERVER_KEY_FIRST_HALF: &'static str = "Genuine Adobe Flash Media Server 001";
pub const RTMP_CLIENT_KEY_FIRST_HALF: &'static str = "Genuine Adobe Flash Player 001";

pub const RTMP_SERVER_KEY: [u8; 68] = [
    0x47, 0x65, 0x6e, 0x75, 0x69, 0x6e, 0x65, 0x20, 0x41, 0x64, 0x6f, 0x62, 0x65, 0x20, 0x46, 0x6c,
    0x61, 0x73, 0x68, 0x20, 0x4d, 0x65, 0x64, 0x69, 0x61, 0x20, 0x53, 0x65, 0x72, 0x76, 0x65, 0x72,
    0x20, 0x30, 0x30, 0x31, // Genuine Adobe Flash Media Server 001
    0xf0, 0xee, 0xc2, 0x4a, 0x80, 0x68, 0xbe, 0xe8, 0x2e, 0x00, 0xd0, 0xd1, 0x02, 0x9e, 0x7e, 0x57,
    0x6e, 0xec, 0x5d, 0x2d, 0x29, 0x80, 0x6f, 0xab, 0x93, 0xb8, 0xe6, 0x36, 0xcf, 0xeb, 0x31, 0xae,
]; // 68
