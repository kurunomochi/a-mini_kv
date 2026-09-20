use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::net::{
    TcpListener, TcpStream,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio::sync::Mutex;
type SharedDatabase = Arc<Mutex<Database>>;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
type BoxError = Box<dyn std::error::Error + Send + Sync>;
#[derive(Debug)]
enum ParseError {
    EmptyCommand,
    UnknownCommand,
    InvalidArguments,
    InvalidFrame,
    InvalidUtf8,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ParseError::EmptyCommand => "empty command",
            ParseError::UnknownCommand => "unknown command",
            ParseError::InvalidArguments => "invalid arguments",
            ParseError::InvalidFrame => "invalid frame",
            ParseError::InvalidUtf8 => "invalid UTF-8",
        };

        formatter.write_str(message)
    }
}

impl std::error::Error for ParseError {}
#[derive(Debug)]
enum Frame {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Vec<Frame>),
}

#[derive(Debug)]
enum Response {
    Ok,
    Value(Option<String>),
    Deleted(bool),
}
#[derive(Debug)]
enum Command {
    SET { key: String, value: String },
    GET { key: String },
    DELETE { key: String },
}
struct Connection {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}
impl Connection {
    fn new(socket: TcpStream) -> Self {
        let (read_half, write_half) = socket.into_split();
        Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        }
    }
    async fn read_line_bytes(&mut self) -> Result<Vec<u8>, BoxError> {
        let mut buffer = Vec::new();

        self.reader.read_until(b'\n', &mut buffer).await?;

        if buffer.len() < 2
            || buffer[buffer.len() - 2] != b'\r'
            || buffer[buffer.len() - 1] != b'\n'
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid RESP line ending",
            )
            .into());
        }

        // 删除末尾的 \r\n
        buffer.truncate(buffer.len() - 2);

        Ok(buffer)
    }

    async fn read_number(&mut self) -> Result<i64, BoxError> {
        let bytes = self.read_line_bytes().await?;

        let text = std::str::from_utf8(&bytes)?;
        let number = text.parse::<i64>()?;

        Ok(number)
    }
    async fn read_frame(&mut self) -> Result<Frame, BoxError> {
        let mut prefix = [0u8; 1];

        self.reader.read_exact(&mut prefix).await?;

        match prefix[0] {
            b'+' => {
                let bytes = self.read_line_bytes().await?;
                let value = String::from_utf8(bytes)?;

                Ok(Frame::SimpleString(value))
            }

            b'-' => {
                let bytes = self.read_line_bytes().await?;
                let value = String::from_utf8(bytes)?;

                Ok(Frame::Error(value))
            }

            b':' => {
                let number = self.read_number().await?;
                Ok(Frame::Integer(number))
            }

            b'$' => {
                let length = self.read_number().await?;

                if length == -1 {
                    return Ok(Frame::BulkString(None));
                }

                if length < 0 {
                    return Err("invalid bulk string length".into());
                }

                let mut data = vec![0u8; length as usize];

                self.reader.read_exact(&mut data).await?;

                let mut crlf = [0u8; 2];
                self.reader.read_exact(&mut crlf).await?;

                if crlf != [b'\r', b'\n'] {
                    return Err("invalid bulk string ending".into());
                }

                Ok(Frame::BulkString(Some(data)))
            }

            b'*' => {
                let length = self.read_number().await?;

                if length < 0 {
                    return Err("null array is not supported".into());
                }

                let mut frames = Vec::new();

                for _ in 0..length {
                    // 递归读取数组中的子 Frame
                    let frame = Box::pin(self.read_frame()).await?;
                    frames.push(frame);
                }

                Ok(Frame::Array(frames))
            }

            _ => Err("unknown RESP frame type".into()),
        }
    }
    async fn write_frame(&mut self, frame: Frame) -> Result<(), BoxError> {
        match frame {
            Frame::SimpleString(value) => {
                self.writer.write_all(b"+").await?;
                self.writer.write_all(value.as_bytes()).await?;
                self.writer.write_all(b"\r\n").await?;
            }

            Frame::Error(value) => {
                self.writer.write_all(b"-").await?;
                self.writer.write_all(value.as_bytes()).await?;
                self.writer.write_all(b"\r\n").await?;
            }

            Frame::Integer(value) => {
                let output = format!(":{}\r\n", value);
                self.writer.write_all(output.as_bytes()).await?;
            }

            Frame::BulkString(None) => {
                self.writer.write_all(b"$-1\r\n").await?;
            }

            Frame::BulkString(Some(data)) => {
                let header = format!("${}\r\n", data.len());

                self.writer.write_all(header.as_bytes()).await?;
                self.writer.write_all(&data).await?;
                self.writer.write_all(b"\r\n").await?;
            }

            Frame::Array(frames) => {
                let header = format!("*{}\r\n", frames.len());
                self.writer.write_all(header.as_bytes()).await?;

                for frame in frames {
                    Box::pin(self.write_frame(frame)).await?;
                }
            }
        }

        self.writer.flush().await?;
        Ok(())
    }
}

struct Database {
    kv: HashMap<String, String>,
}
impl Database {
    fn new() -> Self {
        Database { kv: HashMap::new() }
    }
    fn set(&mut self, key: String, value: String) {
        self.kv.insert(key, value);
    }
    fn get(&self, key: &str) -> Option<&str> {
        self.kv.get(key).map(String::as_str)
    }
    fn delete(&mut self, key: &str) -> bool {
        self.kv.remove(key).is_some()
    }
    fn execute(&mut self, command: Command) -> Response {
        match command {
            Command::SET { key, value } => {
                self.set(key, value);
                Response::Ok
            }
            Command::GET { key } => Response::Value(self.get(&key).map(str::to_owned)),
            Command::DELETE { key } => Response::Deleted(self.delete(&key)),
        }
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let db = Arc::new(Mutex::new(Database::new()));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let db = Arc::clone(&db);

        tokio::spawn(async move {
            if let Err(error) = process(socket, db).await {
                eprintln!("connection error: {error}");
            }
        });
    }
}

async fn process(socket: TcpStream, db: SharedDatabase) -> Result<(), BoxError> {
    let mut connection = Connection::new(socket);

    loop {
        let frame = connection.read_frame().await?;
        let command = frame_to_command(frame)?;

        let response = {
            let mut database = db.lock().await;
            database.execute(command)
        };

        connection.write_frame(response_to_frame(response)).await?;
    }
}
fn frame_to_string(frame: Frame) -> Result<String, ParseError> {
    match frame {
        Frame::BulkString(Some(bytes)) => {
            String::from_utf8(bytes).map_err(|_| ParseError::InvalidUtf8)
        }
        Frame::SimpleString(value) => Ok(value),

        _ => Err(ParseError::InvalidFrame),
    }
}
fn frame_to_command(frame: Frame) -> Result<Command, ParseError> {
    let frames = match frame {
        Frame::Array(frames) => frames,
        _ => return Err(ParseError::InvalidFrame),
    };
    let mut args = Vec::new();
    for frame in frames {
        args.push(frame_to_string(frame)?);
    }
    let command_name = args
        .first()
        .ok_or(ParseError::EmptyCommand)?
        .to_ascii_uppercase();
    match command_name.as_str() {
        "SET" => {
            if args.len() != 3 {
                return Err(ParseError::InvalidArguments);
            }
            Ok(Command::SET {
                key: args[1].clone(),
                value: args[2].clone(),
            })
        }
        "GET" => {
            if args.len() != 2 {
                return Err(ParseError::InvalidArguments);
            }
            Ok(Command::GET {
                key: args[1].clone(),
            })
        }
        "DELETE" | "DEL" => {
            if args.len() != 2 {
                return Err(ParseError::InvalidArguments);
            }
            Ok(Command::DELETE {
                key: args[1].clone(),
            })
        }

        _ => Err(ParseError::UnknownCommand),
    }
}

fn response_to_frame(response: Response) -> Frame {
    match response {
        Response::Ok => Frame::SimpleString("OK".to_string()),
        Response::Value(Some(value)) => Frame::BulkString(Some(value.into_bytes())),
        Response::Value(None) => Frame::BulkString(None),
        Response::Deleted(true) => Frame::Integer(1),
        Response::Deleted(false) => Frame::Integer(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_set_and_get() {
        let mut db = Database::new();
        let command = frame_to_command(Frame::Array(vec![
            Frame::BulkString(Some(b"SET".to_vec())),
            Frame::BulkString(Some(b"name".to_vec())),
            Frame::BulkString(Some(b"shini".to_vec())),
        ]))
        .unwrap();
        let response = db.execute(command);
        assert!(matches!(response, Response::Ok));
        let command = frame_to_command(Frame::Array(vec![
            Frame::BulkString(Some(b"GET".to_vec())),
            Frame::BulkString(Some(b"name".to_vec())),
        ]))
        .unwrap();
        let response = db.execute(command);
        assert!(matches!(response,Response::Value(Some(value))if value =="shini"));
        let command = frame_to_command(Frame::Array(vec![
            Frame::BulkString(Some(b"DEL".to_vec())),
            Frame::BulkString(Some(b"name".to_vec())),
        ]))
        .unwrap();
        let response = db.execute(command);
        assert!(matches!(response, Response::Deleted(true)));
        let command = frame_to_command(Frame::Array(vec![
            Frame::BulkString(Some(b"GET".to_vec())),
            Frame::BulkString(Some(b"name".to_vec())),
        ]))
        .unwrap();
        let response = db.execute(command);
        assert!(matches!(response, Response::Value(None)));
    }
}
