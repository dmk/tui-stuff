use std::io::Cursor;

pub fn play_ogg(bytes: Vec<u8>) -> Result<(), String> {
    let cursor = Cursor::new(bytes);
    let (_stream, handle) = rodio::OutputStream::try_default().map_err(|err| err.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|err| err.to_string())?;
    let source = rodio::Decoder::new(cursor).map_err(|err| err.to_string())?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
