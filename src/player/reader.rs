use std::io::{BufRead, BufReader, Read};
use std::time::Instant;
use usi::EngineCommand;

use crate::error::Error;

pub struct EngineOutput {
    response: Option<EngineCommand>,
    raw_str: String,
    timestamp: Instant,
}

impl EngineOutput {
    pub fn response(&self) -> &Option<EngineCommand> {
        &self.response
    }

    pub fn raw_str(&self) -> &str {
        &self.raw_str
    }

    pub fn timestamp(&self) -> &Instant {
        &self.timestamp
    }
}

pub struct EngineCommandReader<T: Read> {
    receive: BufReader<T>,
    subscribers: Vec<Box<FnMut(&EngineOutput) + Send + Sync>>,
}

impl<T: Read> EngineCommandReader<T> {
    pub fn new(receive: T) -> EngineCommandReader<T> {
        EngineCommandReader {
            receive: BufReader::new(receive),
            subscribers: Default::default(),
        }
    }

    pub fn next(&mut self) -> Result<EngineOutput, Error> {
        match self.next_inner() {
            Ok(output) => {
                for f in self.subscribers.iter_mut() {
                    f(&output);
                }
                Ok(output)
            }
            e @ Err(_) => e,
        }
    }

    fn next_inner(&mut self) -> Result<EngineOutput, Error> {
        let mut buf = String::new();

        loop {
            let bytes_read = self.receive.read_line(&mut buf)?;
            if bytes_read == 0 {
                return Ok(EngineOutput {
                    response: None,
                    raw_str: buf,
                    timestamp: Instant::now(),
                });
            }

            if !buf.trim().is_empty() {
                break;
            }
            buf.clear();
        }

        let res = EngineCommand::parse(&buf)?;
        Ok(EngineOutput {
            response: Some(res),
            raw_str: buf,
            timestamp: Instant::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usi::BestMoveParams;

    #[test]
    fn it_works() {
        let buf = "\nusiok\n\n     readyok\n  bestmove 5e5f\n";

        let mut reader = EngineCommandReader::new(buf.as_bytes());

        let output = reader.next().expect("failed to read the output");
        match *output.response() {
            Some(EngineCommand::UsiOk) => assert!(true),
            ref r => unreachable!("unexpected {:?}", r),
        }
        assert_eq!("usiok\n", output.raw_str());

        let output = reader.next().expect("failed to read the output");
        match *output.response() {
            Some(EngineCommand::ReadyOk) => assert!(true),
            ref r => unreachable!("unexpected {:?}", r),
        }
        assert_eq!("     readyok\n", output.raw_str());

        let output = reader.next().expect("failed to read the output");
        match *output.response() {
            Some(EngineCommand::BestMove(BestMoveParams::MakeMove(_, None))) => assert!(true),
            ref r => unreachable!("unexpected {:?}", r),
        }
        assert_eq!("  bestmove 5e5f\n", output.raw_str());
    }
}
