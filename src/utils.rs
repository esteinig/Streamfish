
use std::io::Write;
use itertools::join;
use log::{LevelFilter, Level};
use env_logger::{Builder, fmt::Color};
use byteorder::{LittleEndian, ByteOrder};

pub fn init_logger() {
    Builder::new()
    .format(|buf, record| {
        let timestamp = buf.timestamp();
    
        let mut red_style = buf.style();
        red_style.set_color(Color::Red).set_bold(true);
        let mut green_style = buf.style();
        green_style.set_color(Color::Green).set_bold(true);
        let mut white_style = buf.style();
        white_style.set_color(Color::White).set_bold(false);
        let mut orange_style = buf.style();
        orange_style.set_color(Color::Rgb(255, 102, 0)).set_bold(true);
        let mut apricot_style = buf.style();
        apricot_style.set_color(Color::Rgb(255, 195, 0)).set_bold(true);
    
        let msg = match record.level(){
            Level::Warn => (orange_style.value(record.level()), orange_style.value(record.args())),
            Level::Info => (green_style.value(record.level()), white_style.value(record.args())),
            Level::Debug => (apricot_style.value(record.level()), apricot_style.value(record.args())),
            Level::Error => (red_style.value(record.level()), red_style.value(record.args())),
            _ => (white_style.value(record.level()), white_style.value(record.args()))
        };
    
        writeln!(
            buf,
            "{} [{}] - {}",
            white_style.value(timestamp),
            msg.0,
            msg.1
        )
    })
    .filter(None, LevelFilter::Info)
    .init();

}

pub fn get_basecall_client_input(id: String, raw_data: Vec<u8>, chunks: usize, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

    // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
    let mut signal_data: Vec<i16> = Vec::new();
    for i in (0..raw_data.len()).step_by(2) {
        signal_data.push(LittleEndian::read_i16(&raw_data[i..]));
    }

    format!("{}::{}::{}::{} {} {} {} {:.1} {:.11} {} {}\n", id, channel, number, chunks, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
}
