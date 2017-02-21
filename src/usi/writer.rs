use std::io::Write;
use shogi::usi::GuiCommand;

use error::Error;

pub struct GuiCommandWriter<T: Write> {
    writer: T,
    subscribers: Vec<Box<FnMut(&GuiCommand, &str) + Send + Sync>>,
}

impl<T: Write> GuiCommandWriter<T> {
    pub fn new(writer: T) -> GuiCommandWriter<T> {
        GuiCommandWriter {
            writer: writer,
            subscribers: Default::default(),
        }
    }

    pub fn send(&mut self, command: &GuiCommand) -> Result<String, Error> {
        let s = format!("{}\n", command);
        try!(self.writer.write_all(&s.as_bytes()));
        try!(self.writer.flush());

        for f in self.subscribers.iter_mut() {
            f(command, s.as_str());
        }

        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let buf: Vec<u8> = Vec::new();
        let mut writer = GuiCommandWriter::new(buf);
        let s = writer.send(&GuiCommand::Usi).expect("failed to write to the buffer");
        assert_eq!("usi\n", s);
        let s = writer.send(&GuiCommand::IsReady).expect("failed to write to the buffer");
        assert_eq!("isready\n", s);
        let s = writer.send(&GuiCommand::SetOption("key".to_string(), Some("val".to_string())))
            .expect("failed to write to the buffer");
        assert_eq!("setoption name key value val\n", s);
    }
}
