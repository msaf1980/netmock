extern crate tokio;

use super::CheckedMockStreamBuilder;

use super::SimpleMockStream;

use std::io::Error;
use std::{
    io::{Read, Write},
    time::Duration,
};

#[test]
fn simple_mockstream_writing() {
    let mut stream = SimpleMockStream::empty();
    stream
        .write_all(b"This is a test of the emergency broadcast system.")
        .unwrap();
    assert!(stream.readed().is_empty());
    let expected = b"This is a test of the emergency broadcast system.";
    assert_eq!(stream.written().to_owned(), expected.to_vec());
    stream.write_all(b"\nEOF\n").unwrap();
    let expected = b"This is a test of the emergency broadcast system.\nEOF\n";
    assert_eq!(stream.written().to_owned(), expected.to_vec());
}

#[test]
fn simple_mockstream_reading() {
    let read = b"Hello\nGoodbye\n";
    let mut stream = SimpleMockStream::new(read.to_vec());
    let mut buf = Vec::<u8>::with_capacity(20);
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, read);
    assert_eq!(readed, read.len());

    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, read);
    assert_eq!(readed, 0);
}

#[test]
fn simple_mockstream() {
    let read = b"Hello\nGoodbye\n";
    let mut stream = SimpleMockStream::new(read.to_vec());
    let mut buf = Vec::<u8>::with_capacity(20);
    buf.resize(6, 0);
    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, b"Hello\n");
    assert_eq!(readed, 6);

    stream.write_all(b"WRITE").unwrap();
    assert_eq!(stream.written().to_owned(), b"WRITE");
    stream.reset_written();
    assert_eq!(stream.written().to_owned(), b"");

    buf.clear();
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"Goodbye\n");
    assert_eq!(readed, read.len() - 6);

    stream.reset_actions();
    buf.clear();
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, read);
    assert_eq!(readed, read.len());
}

#[test]
fn checked_mockstream() {
    let mut stream = CheckedMockStreamBuilder::new()
        .read(b"First\nSecond\n".to_vec())
        .wait(Duration::from_millis(100))
        .read(b"Third\n".to_vec())
        .write(b"Success\n".to_vec())
        .wait(Duration::from_millis(100))
        .write(b"Ping\n".to_vec())
        .write(b"Next\n".to_vec())
        .read(b"Four\n".to_vec())
        .write(b"End\n".to_vec())
        .build();

    let mut buf = Vec::<u8>::with_capacity(20);
    buf.resize(6, 0);
    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, b"First\n");
    assert_eq!(readed, 6);

    let result = stream.write_all(b"Success\n");
    assert!(result.is_err());
    assert_eq!(stream.written(), &vec![]);

    let written = stream.write(b"Success\n").unwrap();
    assert_eq!(written, 0);

    buf.clear();
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"Second\nThird\n");
    assert_eq!(readed, 13);

    buf.clear();
    stream.reset_actions();
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"First\nSecond\nThird\n");
    assert_eq!(readed, 19);

    let result = stream.write_all(b"Missmatch");
    assert!(result.is_err());
    assert_eq!(stream.written(), &vec![]);

    stream.seek_action(3);
    let result = stream.write_all(b"Success\n");
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"Success\n");

    // .wait(Duration::from_millis(100))
    let start = std::time::SystemTime::now();

    let result = stream.write_all(b"Ping\nNext\n");
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"Success\nPing\nNext\n");

    let duration = std::time::SystemTime::now().duration_since(start).unwrap();
    assert!(
        duration > Duration::from_millis(90) && duration < Duration::from_millis(120),
        "{:?}",
        duration
    );

    stream.seek_action(5);
    stream.reset_written();
    let start = std::time::SystemTime::now();
    let result = stream.write_all(b"Ping\nNext\n");
    let duration = std::time::SystemTime::now().duration_since(start).unwrap();
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"Ping\nNext\n");
    assert!(
        duration < Duration::from_millis(1),
        "{:?}",
        duration
    );

    buf.clear();
    let start = std::time::SystemTime::now();
    let readed = stream.read_to_end(&mut buf).unwrap();
    let duration = std::time::SystemTime::now().duration_since(start).unwrap();
    assert_eq!(&buf, b"Four\n");
    assert_eq!(readed, 5);
    assert!(
        duration < Duration::from_millis(1),
        "{:?}",
        duration
    );

    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"Four\n");
    assert_eq!(readed, 0);

    // no more read
    buf.clear();
    buf.resize(2, 0);
    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, &[0, 0]);
    assert_eq!(readed, 0);

    stream.reset_written();
    let result = stream.write_all(b"End\n");
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"End\n");

    // no more
    let result = stream.write(b"EOF\n");
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"End\n");

    stream.reset_written();
    let result = stream.write_all(b"EOF\n");
    assert!(result.is_err());
    assert_eq!(stream.written(), b"");
}


#[test]
fn checked_mockstream_error() {
    let mut stream = CheckedMockStreamBuilder::new()
        .read(b"First\nSecond\n".to_vec())
        .wait(Duration::from_millis(100))
        .write_error(Error::new(std::io::ErrorKind::Other, "write"))
        .write(b"Success\n".to_vec())
        .read_error(Error::new(std::io::ErrorKind::Other, "read"))
        .read(b"Third\n".to_vec())
        .build();

    let mut buf = Vec::<u8>::with_capacity(20);
    buf.resize(6, 0);
    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, b"First\n");
    assert_eq!(readed, 6);

    let result = stream.write_all(b"Success\n");
    assert!(result.is_err());
    assert_eq!(stream.written(), &vec![]);

    buf.clear();
    let readed = stream.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf, b"Second\n");
    assert_eq!(readed, 7);

    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(readed, 0);

    let result = stream.write_all(b"Error\n");
    assert!(result.is_err());
    assert_eq!(stream.written(), &vec![]);

    let result = stream.write_all(b"Success\n");
    assert!(result.is_ok(), "{}", result.err().unwrap());
    assert_eq!(stream.written(), b"Success\n");

    let written = stream.write(b"0\n").unwrap();
    assert_eq!(written, 0);

    let result = stream.read(&mut buf);
    assert!(result.is_err());

    buf.resize(6, 0);
    let readed = stream.read(&mut buf).unwrap();
    assert_eq!(&buf, b"Third\n");
    assert_eq!(readed, 6);
}
